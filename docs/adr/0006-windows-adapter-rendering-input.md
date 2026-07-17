# ADR-006: Windows Adapter — Rendering & Input Strategy

**Status:** Accepted
**Date:** 2026-07-16

## Context

Windows is the first target host OS. We needed to choose the mechanism the Windows
adapter uses to give a workspace its own screen and its own input, for *modern* apps
(Chrome, VS Code — both GPU-composited Chromium).

Several architectures were proposed: hidden desktop, virtual display, per-window
capture, and a custom compositor. This ADR records why the choice is not a matter of
opinion.

### Evidence — measured

We built and ran these. These are results, not predictions.

| Test | Result |
|------|--------|
| **Hidden desktop** (`CreateDesktopW`) + `PrintWindow` capture, Chromium app | **Black frame.** |
| Hidden desktop + `PostMessage` input (WM_MOUSE*/WM_CHAR), Chromium app | **Silently ignored.** |
| Same mechanism, classic Win32 app (Character Map) | Worked — capture and input both. |
| **WSL2** + private X display + Chrome, captured and streamed over WebRTC | **Clear frames**, 1600x900, first frame ~2s. |
| WSL2 workspace, remote input via X11 injection | **Worked** — clicked Google's search box from a Windows browser and typed; autocomplete fired. |
| Arna agent compiled for Linux | Built and ran **unchanged** (`scrap`/`enigo`/`openh264`). |

The hidden-desktop failures are **not bugs**. Windows does not composite non-active
desktops, so there are no pixels to read; and it delivers real input only to the input
desktop. There is no API that changes either.

### Evidence — reasoned, not measured

- **Windows.Graphics.Capture (WGC)** *can* capture a specific window including
  GPU-composited ones (OBS relies on it), even when occluded — so per-window capture is
  solvable. But it requires DWM composition, which non-active desktops do not get, and
  it says nothing about input.
- **Virtual display driver (IddCx)** + Desktop Duplication captures fine and is how
  Sunshine/Parsec work — but a virtual display belongs to the *same session*.
- **An RDP session** genuinely has the primitive we want: its own input queue and its
  own virtual display. On client SKUs it is license-limited to one active session.

### The actual constraint

Rendering was never the wall. Several mechanisms solve rendering.

> **Windows gives one interactive session exactly one input queue and one cursor.**

Two independent cursors in one session does not exist. That is a *session* property, so
no rendering strategy — hidden desktop, virtual display, WGC, or custom compositor —
can address it. A second independent input queue requires a second session, and client
Windows permits one. A VM is a second OS instance, hence a second session.

## Decision

**The Windows adapter creates workspaces backed by WSL2.**

WSL2 is Microsoft's own lightweight VM: it ships with Windows, works on Home edition,
starts in about a second, and has the GPU path available. Per
[ADR-001](0001-product-scope.md) we rent this primitive rather than build one.

Everything a user sees remains ours — our guest image, our compositor and desktop, our
runtime, our app catalog. WSL2 is an invisible boot mechanism behind the adapter
interface ([ADR-005](0005-platform-adapter-architecture.md)), not a user-facing choice.

**Two WSL defaults are security-critical and must be disabled at provisioning:**

| Default | Why it must go |
|---------|----------------|
| `automount` — mounts the host's entire drive at `/mnt/c` | Violates deny-by-default; the workspace would see the whole filesystem |
| `interop` — lets Linux execute Windows binaries (`cmd.exe`) | **Escape hatch straight to the host.** Breaks the isolation claim outright |

Shared folders are then bind-mounted **only** where the host granted them
([ADR-003](0003-hardware-ownership-model.md)).

## Alternatives Considered

**Hidden desktop / "bubble".** Rejected on measured evidence above. Viable only for
classic Win32 apps, which is not our catalog.

**Virtual display (IddCx).** Rejected as primary. Solves rendering, not the input queue;
same session means one cursor.

**Per-window capture (WGC) + custom compositor.** Rejected as primary. Its strongest
form — run apps off-screen on the active desktop, WGC-capture each window, composite
them — solves rendering *and* hides windows from the host, but input still moves the
host's one cursor, and the apps remain on the host's desktop, so isolation fails too.
WGC remains a candidate for *host-side* screen sharing, which is a different feature.

**Windows multi-session (RDS / RDP Wrapper).** Rejected for client hosts. The right
primitive, but Server-only or a `termsrv.dll` patch that breaks on Windows updates —
unshippable. Revisit as a distinct `WindowsRds` adapter for Server hosts, where it is
the *better* answer than WSL2.

**Full VM with a Windows guest.** Rejected as default. It would run real Windows apps,
but costs a Windows license per workspace and much more RAM — and still would not
provide the *host's installed* apps, since a guest has its own disk.

## Consequences

- **Workspace apps are Linux apps from a catalog we curate.** The design note's
  "Host Resources: Installed Apps" is not achievable on Windows Home/Pro by any route
  and must be reworded to "apps the host curates for the workspace."
- **Workspace↔host isolation is VM-grade.** Ordinary malware in a workspace cannot reach
  the host — provided `interop` and `automount` are off. **This adapter prototype depends
  critically on those two settings; they are not the platform's security model.** The
  security model is the obligation set in SPEC §18.3, discharged by adapter isolation,
  policy enforcement, filesystem virtualization, resource isolation, and capability
  enforcement. These two flags are how *this* adapter currently discharges *part* of that
  obligation — an implementation detail that a future Windows adapter may discharge
  entirely differently. Elevating them to "the foundation" would let the implementation
  define the platform, which
  [ADR-007](0007-specification-before-platform-integration.md) forbids. They warrant a
  test that fails the build because the prototype depends on them, not because they are
  load-bearing for the platform.
- **Workspace↔workspace isolation is shared-kernel, not VM-grade.** All WSL2 instances
  share one utility VM and kernel. Separate namespaces mean workspace A cannot *see*
  workspace B, but a kernel exploit would cross. Acceptable under the invited-guests
  threat model in [ADR-003](0003-hardware-ownership-model.md); it must be stated
  plainly and revisited if the audience ever widens to untrusted strangers.
- **Per-workspace resource limits need building.** `.wslconfig` limits are global to the
  whole WSL VM. Per-workspace CPU/RAM caps and live usage require cgroups v2 inside.
- **Hosts whose primitives are closer to our model will need no VM at all**, so this
  adapter carries overhead others will not. That is an *implementation* difference and
  **must not become a product difference**: parity is the goal wherever practical, and
  any unavoidable gap surfaces as an explicit capability
  ([ADR-005](0005-platform-adapter-architecture.md)), never as a visibly different
  product. Users should not be able to tell which adapter they are on.
