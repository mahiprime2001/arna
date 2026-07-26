//go:build windows

package wsl

import (
	"encoding/base64"
	"fmt"
	"strings"
	"time"
)

// A workspace session is the running desktop: a private virtual display, a
// window manager, the app, and a VNC-over-WebSocket bridge the browser renders
// with noVNC. Everything here targets the workspace's OWN display (:1) -- never
// the host's, and never WSL's built-in WSLg bridge, which we actively detach.
//
// The Alpine display stack. Small (musl); ~a few MB installed.
const displayStack = "xvfb x11vnc xterm openbox novnc websockify ttf-dejavu xdotool"

// Ports inside the workspace VM. x11vnc serves RFB on vncPort (localhost only);
// websockify bridges it to wsPort, which WSL2 forwards to Windows localhost.
const (
	vncPort = 5901
	wsPort  = 6080 // one workspace at a time in Stage A; see StartSession
)

// sessionScript brings the desktop up. It encodes three fixes found the hard
// way on real WSL2 (see git history):
//  1. /tmp/.X11-unix is a READ-ONLY WSLg bind-mount from the host, so Xvfb
//     can't create its socket there. Detach it and make a private 1777 dir --
//     which also removes a host-shared path.
//  2. WSLg sets WAYLAND_DISPLAY, which makes x11vnc think it's a Wayland
//     session and quit. Unset it; our display is real X11.
//  3. Wait for the X socket before starting the WM/app/VNC.
//
// It is idempotent: re-running re-uses whatever is already up.
const sessionScript = `#!/bin/sh
# Managed by Arna. Brings the workspace desktop up on a private display, from a
# clean slate every time. Runs everything as plain background jobs and records
# their PIDs, so stop/restart is exact -- no pattern-matching (which self-killed
# the launching shell) and no stale-process races (a leftover Xvfb kept the
# socket dir from being rebuilt).
PORT="${1:-6080}"
APP="${2:-xterm -geometry 100x30+30+30}"
export DISPLAY=:1
unset WAYLAND_DISPLAY          # WSLg sets this; it makes x11vnc quit thinking it is Wayland
mkdir -p /run/arna

# stop any previous session cleanly (pidfiles first, then exact names)
for pf in /run/arna/*.pid; do [ -f "$pf" ] && kill "$(cat "$pf")" 2>/dev/null; done
for n in x11vnc xterm openbox Xvfb; do pkill -x "$n" 2>/dev/null; done
sleep 1; rm -f /run/arna/*.pid

# (1) detach WSLg host bridges; make a private X socket dir (must precede Xvfb,
#     and there is no stale Xvfb now, so recreating the dir is safe)
umount -l /tmp/.X11-unix 2>/dev/null || true
umount -l /mnt/wslg/distro 2>/dev/null || true
umount -l /mnt/wslg 2>/dev/null || true
rm -rf /tmp/.X11-unix /tmp/.X1-lock
mkdir -p /tmp/.X11-unix && chmod 1777 /tmp/.X11-unix

# (2) virtual display
Xvfb :1 -screen 0 1280x800x24 -nolisten tcp >/tmp/xvfb.log 2>&1 & echo $! >/run/arna/xvfb.pid
i=0; while [ ! -S /tmp/.X11-unix/X1 ] && [ $i -lt 40 ]; do sleep 0.25; i=$((i+1)); done
[ -S /tmp/.X11-unix/X1 ] || { echo "ERR: no display"; exit 1; }

# (3) window manager + the app
openbox >/tmp/openbox.log 2>&1 & echo $! >/run/arna/openbox.pid
$APP >/tmp/app.log 2>&1 & echo $! >/run/arna/app.pid

# (4) VNC server on the private display, localhost only (plain bg so we own the pid)
x11vnc -display :1 -forever -shared -rfbport 5901 -localhost -nopw -quiet >/tmp/x11vnc.log 2>&1 &
echo $! >/run/arna/x11vnc.pid

# (5) WebSocket bridge + noVNC web client (plain bg so we own the pid)
websockify --web=/usr/share/novnc "$PORT" localhost:5901 >/tmp/ws.log 2>&1 & echo $! >/run/arna/ws.pid

# wait for the bridge port, then report
i=0; while ! ss -ltn 2>/dev/null | grep -q ":$PORT " && [ $i -lt 40 ]; do sleep 0.25; i=$((i+1)); done
if ss -ltn 2>/dev/null | grep -q ":$PORT "; then echo "session up on $PORT"; else echo "ERR: bridge not up"; fi
`

// SetupDisplay installs the display stack into the workspace, once. Idempotent:
// if the packages are already present it returns quickly.
func (a *Adapter) SetupDisplay(id string) error {
	if ok, err := exists(id); err != nil {
		return err
	} else if !ok {
		return fmt.Errorf("workspace %s does not exist", id)
	}
	// apk add is itself idempotent; --no-progress keeps output small. The stack
	// pulls ~150 packages (python for websockify), so give it several minutes --
	// the default 60s command timeout would kill it mid-install.
	cmd := "apk update -q >/dev/null 2>&1; apk add --no-progress " + displayStack + " >/tmp/apk.log 2>&1; echo done:$?"
	out, err := insideFor(id, 8*time.Minute, "sh", "-c", cmd)
	if err != nil {
		return fmt.Errorf("install display stack: %w", err)
	}
	if !strings.Contains(out, "done:0") {
		tail, _ := inside(id, "sh", "-c", "tail -5 /tmp/apk.log")
		return fmt.Errorf("install display stack failed: %s", strings.TrimSpace(tail))
	}
	return nil
}

// SessionInfo is what the caller needs to show the workspace in a browser.
type SessionInfo struct {
	WSPort int    `json:"wsPort"` // reachable at http://localhost:<WSPort> on the host
	URL    string `json:"url"`    // the noVNC page, ready to autoconnect
}

// StartSession brings the desktop up and returns where to view it. The app is a
// shell command run inside the workspace (defaults to a terminal). Requires
// SetupDisplay to have run.
func (a *Adapter) StartSession(id, app string) (SessionInfo, error) {
	var zero SessionInfo
	if app == "" {
		app = "xterm -geometry 100x30+30+30"
	}

	// Deploy the launcher. We base64-encode it in Go and decode inside the
	// workspace: a quoted heredoc's escaping does NOT survive the
	// Windows -> wsl.exe -> sh layers (it silently expanded $VAR to empty),
	// whereas base64 has no shell-special characters and passes through intact.
	enc := base64.StdEncoding.EncodeToString([]byte(sessionScript))
	deploy := "echo " + enc + " | base64 -d > /usr/local/bin/arna-session.sh && chmod +x /usr/local/bin/arna-session.sh"
	if _, err := inside(id, "sh", "-c", deploy); err != nil {
		return zero, fmt.Errorf("deploy session script: %w", err)
	}

	launch := fmt.Sprintf("setsid /usr/local/bin/arna-session.sh %d %q >/tmp/session.log 2>&1 < /dev/null", wsPort, app)
	if _, err := inside(id, "sh", "-c", launch); err != nil {
		return zero, fmt.Errorf("launch session: %w", err)
	}

	// Wait until the bridge port is actually listening inside the workspace.
	ok := false
	for i := 0; i < 30; i++ {
		out, _ := inside(id, "sh", "-c",
			fmt.Sprintf("{ ss -ltn 2>/dev/null || netstat -ltn; } | grep -q ':%d ' && echo up", wsPort))
		if strings.Contains(out, "up") {
			ok = true
			break
		}
		time.Sleep(400 * time.Millisecond)
	}
	if !ok {
		log, _ := inside(id, "sh", "-c", "cat /tmp/session.log /tmp/x11vnc.log 2>/dev/null | tail -8")
		return zero, fmt.Errorf("session did not come up: %s", strings.TrimSpace(log))
	}

	return SessionInfo{
		WSPort: wsPort,
		URL:    fmt.Sprintf("http://localhost:%d/vnc.html?autoconnect=1&resize=scale", wsPort),
	}, nil
}

// StopSession tears the desktop down but leaves the workspace itself running.
// It kills by recorded PID, never by pattern -- `pkill -f websockify` would
// match the very shell running this command (its argv contains "websockify")
// and terminate it before finishing.
func (a *Adapter) StopSession(id string) error {
	_, err := inside(id, "sh", "-c",
		`for f in /run/arna/*.pid; do [ -f "$f" ] && kill "$(cat "$f")" 2>/dev/null; done; `+
			`for n in x11vnc xterm openbox Xvfb; do pkill -x "$n" 2>/dev/null; done; `+
			`rm -f /run/arna/*.pid; true`)
	return err
}
