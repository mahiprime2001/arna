# WSE Status — Windows Reference Implementation

The project dashboard. Windows is the **reference platform**: the aim is feature
completeness on Windows until `run_all()` is fully green, *before* a second
adapter is written. Every future adapter is measured against it.

Legend: ✅ done · ⏳ next · – not started.

## Dashboard

Two Windows adapters: **WSL** (`SealedVm` isolation, Linux apps — proved the
contract/runtime/adapter boundaries) and **Native** (`DesktopProfile` isolation,
native Windows apps, no WSL — the forward direction). Native is the reference
platform going forward.

| Area          | Contract | Mock | Win/WSL | Win/Native |
| ------------- | :------: | :--: | :-----: | :--------: |
| Core          |    ✅    |  ✅  |   ✅    |    ✅      |
| Isolation     |    ✅    |  ✅  |   ✅    |    ✅ (DesktopProfile) |
| Runtime       |    ✅    |  ✅  |   ✅    |    ✅      |
| Events        |    ✅    |  ✅  |   ✅    |    ✅      |
| Identity      |    ✅    |  ✅  |   ✅    |    ✅      |
| Permissions   |    ✅    |  ✅  |   ✅    |    ✅      |
| Conformance   |    ✅    |  ✅  |   ✅    |    ✅      |
| Applications  |    ✅    |  ✅  |   ✅    |    ✅      |
| Windows       |    ✅    |  ✅  |   ✅    |    ✅      |
| Clipboard     |    ✅    |  ✅  |   ⏳    |    ⏳      |
| Storage       |    ✅    |  ✅  |   ⏳    |    ⏳      |
| Devices       |    ✅    |  ✅  |   ⏳    |    ⏳      |
| Network       |    ⏳    |  ⏳  |   ⏳    |    ⏳      |

"Windows-native complete" = every row above ✅ through Devices, `run_all` fully
green. Native `run_all` is **21/21 live** (real Windows desktops + real isolated
browser instances: Core 12 + Applications 7 + Windows 2, ~23s, zero leftover
processes/desktops/profiles).

Native services (adapter-internal, per platform-services framing): Application
service (launch/stop/enumerate via CreateProcessW on the desktop), Window service
(enumerate/focus/close via EnumDesktopWindows), Browser-profile manager (fresh
isolated profile per instance), Catalog (browser-first; apps tagged Certified/
Compatible/Experimental — apps we cannot isolate are simply absent → NotFound).

## Conformance counts (live)

| Suite | checks | Mock | Windows (live, real WSL2) |
|-------|:---:|:---:|:---:|
| Core (lifecycle, isolation, events, identity, runtime) | 12 | ✅ | ✅ |
| Applications (lifecycle: descriptor→instance) | 7 | ✅ | ✅ |
| Windows | 2 | ✅ | ✅ |
| Clipboard | 6 | ✅ | ⏳ |
| Storage | 8 | ✅ | – |
| Devices | 9 | ✅ | – |
| **run_all** | **44** | **44/44** | **21/21** (Core+Apps+Windows) |

- **Mock** — reference implementation; declares everything; 44/44. Runtime is an
  in-memory `mock v1.0.0` providing the full set.
- **Windows** — the reference platform. 21/21 live (Core+Applications+Windows),
  launching real apps, zero orphan distros. Clipboard is wired (adapter +
  `wse-linux-x11` v1.1.0 clipboard service) with live validation in progress.

## Runtimes (the second extension point)

| Runtime | Version | Provides inside | Windows adapter run_all |
|---------|---------|-----------------|--------------------------|
| [wse-linux-x11](../runtimes/wse-linux-x11/) | 1.1.0 | Applications, Windows, Clipboard | live |
| [wse-lite](../runtimes/wse-lite/) | 1.0.0 | (none) | 12/12 live (core only) |

The **same** Windows adapter conforms on both — different runtime, different
effective set (`adapter ∩ runtime`), no code change. See
[core/runtime.md](core/runtime.md) and [../runtimes/README.md](../runtimes/README.md).

## Roadmap (Windows-first)

1. **Phase 1 — Windows Core** ✅ — lifecycle, isolation, runtime, Applications,
   Windows, conformance, runtime abstraction.
2. **Phase 2 — Windows Capability Completion** *(current)* — finish every
   capability on Windows until `run_all` is fully green. Each follows the proven
   pattern: runtime support → adapter bridge → conformance → live validation → no
   contract changes. Order: Clipboard → Storage → Devices (→ Network if in v1).
3. **Phase 3 — Windows Stabilization** — performance, reliability, crash recovery,
   resource-leak and long-running/concurrent-workspace tests, runtime-upgrade and
   stress/error-injection testing, logging & diagnostics. Target: hundreds of
   create/destroy cycles with no leaks or degradation.
4. **Phase 4 — Windows Reference Implementation v1.0** — all planned capabilities
   implemented, `run_all` fully green, no architectural workarounds, stable
   runtime image(s) + adapter + conformance, docs complete.
5. **Phase 5 — Second platform** (Linux/macOS) — only after Windows is mature.
   The question shifts from "how should this work?" to "can another platform
   satisfy the same standard?" — a much lower-risk project.

## Project rule (in force)

> **No new architectural work unless the Windows implementation exposes a genuine
> gap in the contract.**

The contract has survived multiple implementations, a runtime abstraction, and a
real OS. Effort now shifts from *designing WSE* to *building the Windows reference
implementation* until it is feature-complete and production-ready. Contract edits
from here require a logged, classified justification (ADR-008 Rule 2 /
`runtimes/wse-linux-x11/ENGINEERING_LOG.md`).
