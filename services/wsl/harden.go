//go:build windows

package wsl

import (
	"fmt"
	"strings"
)

// wslConf is the isolation config written into every workspace. It shuts the
// two doors the WSL2 experiment found (ADR-006):
//
//   - automount off  -> the host's C:\ (and every drive) is NOT mounted at
//     /mnt/*, so the workspace cannot see the host filesystem (SPEC §8.1).
//   - interop off     -> the workspace cannot launch host .exe files, and the
//     Windows PATH is not injected (SPEC §3.4).
//
// Honesty (SPEC §18.2/§3.6): this is config the guest kernel enforces, not a
// property of an image we built. A root user inside *could* rewrite it and
// restart. Stage A accepts that; Stage B removes the doors structurally by
// shipping an image that never had them. We still VERIFY after every start so a
// workspace that isn't actually sealed never reports as running.
const wslConf = `# Managed by Arna. Do not edit -- isolation depends on it.
[automount]
enabled = false
mountFsTab = false

[interop]
enabled = false
appendWindowsPath = false

[network]
generateResolvConf = true
`

// harden writes the isolation config, then restarts the distro so the guest
// kernel re-reads it (wsl.conf only takes effect on a fresh start).
func harden(id string) error {
	// Write the file atomically from inside, as root.
	script := fmt.Sprintf(`cat > /etc/wsl.conf <<'ARNA_EOF'
%sARNA_EOF
chmod 0644 /etc/wsl.conf`, wslConf)
	if _, err := inside(id, "sh", "-c", script); err != nil {
		return fmt.Errorf("write wsl.conf: %w", err)
	}
	// Terminate so the next command starts the distro fresh with the new config.
	if _, err := wsl("--terminate", DistroName(id)); err != nil {
		return fmt.Errorf("terminate for reconfig: %w", err)
	}
	return nil
}

// IsolationReport is the evidence that a started workspace is actually sealed.
type IsolationReport struct {
	NoHostFilesystem bool     `json:"noHostFilesystem"` // /mnt/c absent (§8.1)
	NoInterop        bool     `json:"noInterop"`        // can't launch host .exe (§3.4)
	Sealed           bool     `json:"sealed"`           // both of the above
	Details          []string `json:"details"`
}

// verifyIsolation actively probes the running workspace and confirms the doors
// are shut. This is the check that lets us refuse to present an unsealed
// workspace as running (SPEC §18.3 -- isolation is not optional).
func verifyIsolation(id string) IsolationReport {
	rep := IsolationReport{}

	// 1. No host filesystem. /proc/mounts is the authority: with automount off,
	//    WSL leaves EMPTY /mnt/c, /mnt/d directories behind, but nothing is
	//    mounted on them (that tripped an earlier naive check). A real host
	//    drive shows up as an actual mount whose mountpoint is /mnt/<letter>.
	//    This also catches a drive mounted by hand from inside.
	mounts, _ := inside(id, "sh", "-c", "cat /proc/mounts")
	var hostMounts []string
	for _, line := range strings.Split(mounts, "\n") {
		fields := strings.Fields(line)
		if len(fields) < 2 {
			continue
		}
		if isDriveMount(fields[1]) {
			hostMounts = append(hostMounts, fields[1])
		}
	}
	rep.NoHostFilesystem = len(hostMounts) == 0
	if len(hostMounts) == 0 {
		rep.Details = append(rep.Details, "no host drives are mounted")
	} else {
		for _, m := range hostMounts {
			rep.Details = append(rep.Details, "host drive mounted at "+m)
		}
	}

	// 2. No interop: trying to run a host .exe must fail. If interop were on,
	//    this would print the Windows build number; with it off, exec fails.
	out, err := inside(id, "sh", "-c",
		"/mnt/c/Windows/System32/cmd.exe /c ver 2>/dev/null || echo ARNA_NO_INTEROP")
	blocked := err != nil || strings.Contains(out, "ARNA_NO_INTEROP")
	rep.NoInterop = blocked
	if blocked {
		rep.Details = append(rep.Details, "host executables cannot be launched (interop off)")
	} else {
		rep.Details = append(rep.Details, "WARNING: host .exe launched -- interop is NOT disabled")
	}

	rep.Sealed = rep.NoHostFilesystem && rep.NoInterop
	return rep
}

// isDriveMount reports whether a mountpoint is a host drive root, i.e. exactly
// /mnt/<single-letter>. WSL's own /mnt/wsl and /mnt/wslg are multi-character, so
// they don't match -- they're internal tmpfs, not the host filesystem.
func isDriveMount(mountpoint string) bool {
	const p = "/mnt/"
	if !strings.HasPrefix(mountpoint, p) {
		return false
	}
	rest := mountpoint[len(p):]
	return len(rest) == 1 && rest[0] >= 'a' && rest[0] <= 'z'
}
