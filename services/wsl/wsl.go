//go:build windows

// Package wsl is Arna's Stage A workspace adapter: it creates and controls a
// real, isolated Linux workspace using WSL2 -- the hypervisor-backed VM that
// ships with Windows. Nothing here trusts software inside the workspace; the
// isolation is verified after every start (SPEC §3.4, §3.6).
//
// Stage A honestly does NOT achieve everything the spec asks -- notably local-
// network blocking (§13.2), which default WSL2 NAT doesn't give us. Those gaps
// are why Stage B ships our own image. What Stage A DOES enforce and verify:
// no host filesystem (no /mnt/c) and no interop (can't launch host .exe).
package wsl

import (
	"bytes"
	"context"
	"fmt"
	"os/exec"
	"strings"
	"time"
	"unicode/utf16"
)

// distroPrefix marks every distro Arna owns, so we never touch a user's own
// WSL installs (their Ubuntu, docker-desktop, etc.).
const distroPrefix = "arna-ws-"

// DistroName is the WSL distro name for a workspace id.
func DistroName(id string) string { return distroPrefix + id }

// run executes wsl.exe (or any command) with a timeout, returning decoded
// stdout. WSL's *management* output (--list, --status) is UTF-16LE; output from
// `wsl -d <name> -- <cmd>` (the Linux side) is normal UTF-8. decodeWSL handles
// both.
func run(ctx context.Context, name string, args ...string) (string, error) {
	cmd := exec.CommandContext(ctx, name, args...)
	var out, errb bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &errb
	err := cmd.Run()
	s := decodeWSL(out.Bytes())
	if err != nil {
		msg := strings.TrimSpace(decodeWSL(errb.Bytes()))
		if msg == "" {
			msg = err.Error()
		}
		return s, fmt.Errorf("%s: %s", strings.Join(append([]string{name}, args...), " "), msg)
	}
	return s, nil
}

// wsl runs wsl.exe with a default timeout.
func wsl(args ...string) (string, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()
	return run(ctx, "wsl.exe", args...)
}

// exec runs a command *inside* a workspace as root.
func inside(id string, argv ...string) (string, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()
	full := append([]string{"-d", DistroName(id), "--user", "root", "--"}, argv...)
	return run(ctx, "wsl.exe", full...)
}

// decodeWSL turns wsl.exe output into a normal string. It detects UTF-16LE
// (which wsl.exe emits for management commands) by the tell-tale interleaved
// NUL bytes and decodes it; otherwise it returns the bytes as-is.
func decodeWSL(b []byte) string {
	// Strip a UTF-16LE BOM if present.
	if len(b) >= 2 && b[0] == 0xFF && b[1] == 0xFE {
		b = b[2:]
	}
	if looksUTF16LE(b) {
		u := make([]uint16, 0, len(b)/2)
		for i := 0; i+1 < len(b); i += 2 {
			u = append(u, uint16(b[i])|uint16(b[i+1])<<8)
		}
		return string(utf16.Decode(u))
	}
	return string(b)
}

func looksUTF16LE(b []byte) bool {
	if len(b) < 2 {
		return false
	}
	// ASCII-range text as UTF-16LE has a NUL in every high byte. Sample a few.
	zeros, checked := 0, 0
	for i := 1; i < len(b) && checked < 16; i += 2 {
		checked++
		if b[i] == 0 {
			zeros++
		}
	}
	return checked > 0 && zeros*2 >= checked
}

// wslAvailable reports whether wsl.exe exists and WSL2 is usable at all.
func wslAvailable() bool {
	if _, err := exec.LookPath("wsl.exe"); err != nil {
		return false
	}
	_, err := wsl("--status")
	return err == nil
}

// List returns the names of the Arna-owned workspace distros currently
// registered. A user's own distros are filtered out by the prefix.
func List() ([]string, error) {
	out, err := wsl("--list", "--quiet")
	if err != nil {
		return nil, err
	}
	var names []string
	for _, line := range strings.Split(out, "\n") {
		n := strings.TrimSpace(strings.ReplaceAll(line, "\x00", ""))
		if strings.HasPrefix(n, distroPrefix) {
			names = append(names, n)
		}
	}
	return names, nil
}

// exists reports whether a given workspace's distro is registered.
func exists(id string) (bool, error) {
	names, err := List()
	if err != nil {
		return false, err
	}
	want := DistroName(id)
	for _, n := range names {
		if n == want {
			return true, nil
		}
	}
	return false, nil
}
