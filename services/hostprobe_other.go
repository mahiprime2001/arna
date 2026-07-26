//go:build !windows

// Non-Windows hosts. Linux and macOS have real isolation stories (native
// displays; Apple Virtualization), but their adapters aren't built yet, so the
// probe reports honestly rather than claiming a capability we can't back.
package main

import "runtime"

func probeHost() HostReport {
	r := HostReport{Platform: runtime.GOOS, Checks: []HostCheck{}}
	r.add("adapter", false, req,
		"",
		"No workspace adapter for this platform yet. Isolation can't be provided here today.")
	r.finish()
	// finish() will mark this incapable via the failed required check without a
	// hypervisor pass -- which is the honest answer until the adapter exists.
	r.Summary = "Workspaces aren't supported on " + runtime.GOOS + " yet."
	return r
}
