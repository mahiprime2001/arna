// Host capability report -- the platform-neutral shape and the verdict rule.
// Per-OS probing lives in hostprobe_<os>.go; this file owns the type they fill
// and the single rule that turns checks into a verdict (SPEC §18.3).
package main

import (
	"math"
	"net/http"
)

// A check is either required for isolation or an optional capability.
const (
	req = true  // a MUST: isolation depends on it (SPEC §18.3)
	opt = false // a capability that MAY vary per machine (SPEC §18.2)
)

type HostCheck struct {
	ID       string `json:"id"`
	OK       bool   `json:"ok"`
	Required bool   `json:"required"`
	Detail   string `json:"detail"`
}

type HostReport struct {
	Platform string      `json:"platform"`
	Cores    int         `json:"cores"`
	MemoryGB float64     `json:"memoryGb"`
	Checks   []HostCheck `json:"checks"`

	// Verdict is the whole point:
	//   capable     -- can run an isolated workspace right now
	//   needs-setup -- capable of isolation, but a one-time enable/install is needed
	//   incapable   -- cannot isolate on this hardware; MUST NOT run workspaces
	Verdict string `json:"verdict"`
	Summary string `json:"summary"`
}

func (r *HostReport) add(id string, ok, required bool, okMsg, failMsg string) {
	d := okMsg
	if !ok {
		d = failMsg
	}
	r.Checks = append(r.Checks, HostCheck{ID: id, OK: ok, Required: required, Detail: d})
}

// finish derives the verdict. The rule is deliberately strict: if any REQUIRED
// isolation check fails, the machine is not "capable" -- there is no
// partial-isolation tier (SPEC §18.3). We only distinguish "incapable" (the
// hardware can't) from "needs-setup" (it can, once a feature is switched on).
func (r *HostReport) finish() {
	hardwareCanIsolate := true // is the *hardware* capable at all?
	requiredAllMet := true     // are all required checks satisfied *right now*?

	for _, c := range r.Checks {
		if !c.Required {
			continue
		}
		if !c.OK {
			requiredAllMet = false
			// A missing hypervisor is a hardware/firmware gate. A disabled
			// Windows feature is not -- it's a switch. Only the former makes
			// the machine fundamentally incapable.
			if c.ID == "hypervisor" {
				hardwareCanIsolate = false
			}
		}
	}

	switch {
	case requiredAllMet:
		r.Verdict = "capable"
		r.Summary = "This machine can host an isolated workspace."
	case hardwareCanIsolate:
		r.Verdict = "needs-setup"
		r.Summary = "This machine can host workspaces after a one-time setup step."
	default:
		r.Verdict = "incapable"
		r.Summary = "This machine can't provide hardware isolation, so it won't run workspaces."
	}
}

func round1(f float64) float64 { return math.Round(f*10) / 10 }

// GET /api/host -- report whether this machine can run a workspace. Read-only;
// it changes nothing about the host.
func hostCapabilities(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		fail(w, http.StatusMethodNotAllowed, "GET only")
		return
	}
	writeJSON(w, http.StatusOK, probeHost())
}
