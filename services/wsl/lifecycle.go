//go:build windows

package wsl

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
)

// Stage A boots a tiny Alpine rootfs (~3 MB) purely so the pipeline is real and
// testable today. It is NOT the Stage B image -- it's a stand-in that proves
// create -> harden -> verify -> run -> destroy works end to end. Swapping it for
// our own image is a Stage B change behind this same package's API.
const (
	alpineURL    = "https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/x86_64/alpine-minirootfs-3.20.3-x86_64.tar.gz"
	alpineSHA256 = "" // pinned below once fetched; empty = accept + record
)

// Adapter is Arna's handle to WSL2. dataDir holds per-workspace VM disks and
// the cached rootfs.
type Adapter struct {
	dataDir string
}

// New returns an adapter rooted at dataDir, or an error if this machine can't
// run WSL2 workspaces at all (the caller should already have run the host
// probe; this is the adapter's own refusal, per SPEC §18.3).
func New(dataDir string) (*Adapter, error) {
	if !wslAvailable() {
		return nil, fmt.Errorf("WSL2 is not available on this machine")
	}
	if err := os.MkdirAll(dataDir, 0o755); err != nil {
		return nil, err
	}
	return &Adapter{dataDir: dataDir}, nil
}

// rootfs returns a local path to the (cached) workspace rootfs, downloading it
// once. Kept in dataDir so it's fetched a single time per machine.
func (a *Adapter) rootfs() (string, error) {
	dst := filepath.Join(a.dataDir, "rootfs-alpine.tar.gz")
	if fi, err := os.Stat(dst); err == nil && fi.Size() > 0 {
		return dst, nil
	}
	resp, err := http.Get(alpineURL)
	if err != nil {
		return "", fmt.Errorf("download rootfs: %w", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != 200 {
		return "", fmt.Errorf("download rootfs: HTTP %d", resp.StatusCode)
	}
	tmp := dst + ".part"
	f, err := os.Create(tmp)
	if err != nil {
		return "", err
	}
	h := sha256.New()
	if _, err := io.Copy(io.MultiWriter(f, h), resp.Body); err != nil {
		f.Close()
		return "", err
	}
	f.Close()
	if alpineSHA256 != "" && hex.EncodeToString(h.Sum(nil)) != alpineSHA256 {
		os.Remove(tmp)
		return "", fmt.Errorf("rootfs checksum mismatch")
	}
	if err := os.Rename(tmp, dst); err != nil {
		return "", err
	}
	return dst, nil
}

// Provision creates a new, hardened, verified-sealed workspace for id. On
// success the workspace exists (registered) and has passed isolation checks; it
// is left terminated (Created/Saved), not running.
func (a *Adapter) Provision(id string) (IsolationReport, error) {
	var zero IsolationReport
	if ok, err := exists(id); err != nil {
		return zero, err
	} else if ok {
		return zero, fmt.Errorf("workspace %s already exists", id)
	}

	root, err := a.rootfs()
	if err != nil {
		return zero, err
	}
	instDir := filepath.Join(a.dataDir, DistroName(id))
	if err := os.MkdirAll(instDir, 0o755); err != nil {
		return zero, err
	}

	// Import the rootfs as a fresh WSL2 distro.
	if _, err := wsl("--import", DistroName(id), instDir, root, "--version", "2"); err != nil {
		return zero, fmt.Errorf("import: %w", err)
	}

	// Shut the doors, then confirm they're shut on a fresh start.
	if err := harden(id); err != nil {
		_ = a.Destroy(id) // don't leave a half-configured distro around
		return zero, err
	}
	rep := verifyIsolation(id)
	if !rep.Sealed {
		// SPEC §18.3: an environment that isn't isolated is not a workspace.
		// Refuse it rather than hand back something that only looks sealed.
		_ = a.Destroy(id)
		return rep, fmt.Errorf("workspace failed isolation checks; refusing to run")
	}

	// Leave it stopped -- provisioning is not starting.
	_, _ = wsl("--terminate", DistroName(id))
	return rep, nil
}

// Start boots the workspace and re-verifies isolation before reporting it as
// running. Verification on every start is deliberate: config can drift.
func (a *Adapter) Start(id string) (IsolationReport, error) {
	if ok, err := exists(id); err != nil {
		return IsolationReport{}, err
	} else if !ok {
		return IsolationReport{}, fmt.Errorf("workspace %s does not exist", id)
	}
	// A trivial command starts the distro.
	if _, err := inside(id, "true"); err != nil {
		return IsolationReport{}, fmt.Errorf("start: %w", err)
	}
	rep := verifyIsolation(id)
	if !rep.Sealed {
		_, _ = wsl("--terminate", DistroName(id))
		return rep, fmt.Errorf("workspace is not sealed; stopped it")
	}
	return rep, nil
}

// Stop terminates the workspace's VM. State inside a Saved workspace persists on
// its disk; a Temporary one is destroyed by the caller via Destroy.
func (a *Adapter) Stop(id string) error {
	_, err := wsl("--terminate", DistroName(id))
	return err
}

// Destroy unregisters the workspace and deletes its disk irrecoverably
// (SPEC §5.5).
func (a *Adapter) Destroy(id string) error {
	_, _ = wsl("--terminate", DistroName(id))
	if _, err := wsl("--unregister", DistroName(id)); err != nil {
		return err
	}
	return os.RemoveAll(filepath.Join(a.dataDir, DistroName(id)))
}

// Status is a light liveness probe used by the caller to map to spec states.
type Status struct {
	Exists  bool   `json:"exists"`
	Running bool   `json:"running"`
	Kernel  string `json:"kernel"`
}

func (a *Adapter) Status(id string) (Status, error) {
	ok, err := exists(id)
	if err != nil || !ok {
		return Status{Exists: ok}, err
	}
	// Ask the running list; if it's there, it's running.
	running, _ := wsl("--list", "--running", "--quiet")
	st := Status{Exists: true, Running: contains(running, DistroName(id))}
	if st.Running {
		k, _ := inside(id, "sh", "-c", "uname -r")
		st.Kernel = strings.TrimSpace(k)
	}
	return st, nil
}

// contains reports whether needle is one of the (NUL-and-space-stripped) lines
// of haystack -- wsl.exe list output can carry stray NULs from UTF-16 decoding.
func contains(haystack, needle string) bool {
	for _, line := range strings.Split(haystack, "\n") {
		if strings.TrimSpace(strings.ReplaceAll(line, "\x00", "")) == needle {
			return true
		}
	}
	return false
}
