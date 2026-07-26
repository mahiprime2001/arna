//go:build windows

// Windows host capability probe. Answers one question, honestly (SPEC §18.3):
// can THIS machine make a properly isolated workspace? Isolation is not a
// capability that may be faked or degraded -- a host that cannot enforce it
// must be told "no", not handed a box that only looks sealed.
//
// This only READS the machine's state. It changes nothing, needs no admin, and
// never launches a VM. Enabling a disabled feature is a separate, explicit,
// user-approved step -- not something a probe does behind the user's back.
package main

import (
	"os/exec"
	"strconv"
	"strings"
)

// ps runs a PowerShell one-liner and returns its trimmed stdout. Any failure
// (PowerShell missing, access denied, timeout) collapses to "" so a check reads
// as "unknown/absent" rather than crashing the probe.
func ps(script string) string {
	out, err := exec.Command(
		"powershell", "-NoProfile", "-NonInteractive", "-Command", script,
	).Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(out))
}

func probeHost() HostReport {
	r := HostReport{Platform: "windows", Checks: []HostCheck{}}

	// 1. Is a hypervisor running? On Win10/11 with VBS this is usually already
	//    true. Without it, no hardware-isolated VM is possible at all.
	hyp := ps(`(Get-CimInstance Win32_ComputerSystem).HypervisorPresent`)
	hypOn := strings.EqualFold(hyp, "True")
	r.add("hypervisor", hypOn, req,
		"A hypervisor is running -- hardware isolation is available.",
		"No hypervisor is running. Turn on virtualization in your BIOS/UEFI.")

	// 2. VirtualMachinePlatform -- the Windows feature the utility-VM path
	//    (WSL2 / Host Compute Service) is built on. This is the one that
	//    actually gates Stage A.
	vmp := ps(`(Get-CimInstance -ClassName Win32_OptionalFeature -Filter "Name='VirtualMachinePlatform'").InstallState`)
	vmpOn := vmp == "1"
	r.add("vm_platform", vmpOn, req,
		"VirtualMachinePlatform is enabled.",
		"VirtualMachinePlatform is off. Enable it once (needs admin + one reboot).")

	// 3. Host Compute Service present -- the API that creates the VM. Its
	//    binaries ship with the OS; their presence means the Stage-B path
	//    (our own image, no WSL) is reachable on this box.
	hcs := ps(`Test-Path "$env:WINDIR\System32\vmcompute.exe"`)
	hcsOn := strings.EqualFold(hcs, "True")
	r.add("host_compute", hcsOn, opt,
		"Host Compute Service present -- we can create our own VM later.",
		"Host Compute Service not found -- Stage B (own image) unavailable; WSL2 still works.")

	// 4. WSL2 present -- the Stage-A adapter's substrate. Optional because
	//    Stage B won't need it, but its absence means Stage A can't run today.
	wsl := ps(`if (Get-Command wsl.exe -ErrorAction SilentlyContinue) { 'yes' } else { 'no' }`)
	wslOn := wsl == "yes"
	r.add("wsl2", wslOn, opt,
		"WSL2 is available -- the Stage A workspace adapter can run.",
		"WSL2 not installed -- needed for the current (Stage A) workspaces.")

	// 5. Enough headroom to actually host a workspace. Soft check: informs the
	//    verdict's advice, never blocks isolation itself.
	cores, _ := strconv.Atoi(ps(`(Get-CimInstance Win32_ComputerSystem).NumberOfLogicalProcessors`))
	memGB := 0.0
	if b, err := strconv.ParseFloat(ps(`(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory`), 64); err == nil {
		memGB = b / (1024 * 1024 * 1024)
	}
	r.Cores = cores
	r.MemoryGB = round1(memGB)
	enough := cores >= 4 && memGB >= 7.5
	r.add("resources", enough, opt,
		"Enough CPU and memory to host a workspace comfortably.",
		"Limited CPU/memory -- workspaces will run but keep to one at a time.")

	r.finish()
	return r
}
