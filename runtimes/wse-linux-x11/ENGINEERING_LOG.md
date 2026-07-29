# Runtime v1 + Windows Applications — Engineering Log

Every deviation classified: `adapter-bug | runtime-issue | conformance-issue |
spec-ambiguity | platform-limitation`. The contract only changes for the last two,
and only after being written down here first.

## Acceptance criteria (must all hold)

1. Runtime reproducibility — `build.sh` yields the same manifest structure; unique
   immutable version + digest; no manual steps after image creation.
2. Adapter orchestration only — never installs/configures/patches inside the
   runtime. Only: import, create, attest, launch, destroy.
3. Applications conformance — `run_applications` passes on Windows via the image,
   nothing skipped or relaxed.
4. Contract purity — CONTRACT.md, capability specs, errors, events, identity,
   lifecycle all unchanged.
5. Event integrity — a real app produces the *same* contract events as the mock.
6. Cleanup — no orphan distros, processes, mutations, or state beyond contract.

## Milestone statement (the thing we're proving)

> The first real runtime satisfied the WSE Applications contract on a production OS
> without changing the specification.

---

## ✅ MILESTONE ACHIEVED

> The first real runtime (`wse-linux-x11` v1.0.0) satisfied the WSE Applications
> contract on a production OS (Windows/WSL2) **without changing the specification.**

Live result: **21/21** (`run_core` 12 + `run_applications` 7 + `run_windows` 2)
against real WSL2, launching real xterm applications, zero orphan distros.

Acceptance criteria — all met:

1. **Runtime reproducibility** ✓ — `build.sh` yields a fixed manifest structure,
   a unique immutable version + digest (`sha256:34be3169…`), no manual steps.
2. **Adapter orchestration only** ✓ — the adapter imports the image and calls
   runtime-provided scripts (`start-display.sh`, `launch.sh`); it never installs,
   configures, or patches inside the runtime. It carries no app knowledge.
3. **Applications conformance** ✓ — 21/21, nothing skipped or relaxed.
4. **Contract purity** ✓ — CONTRACT.md, capability specs, errors, events,
   identity, lifecycle all unchanged (`git status` shows zero `contract/` edits
   during this phase).
5. **Event integrity** ✓ — events are the engine's; the adapter is mechanical, so
   a real app emits the same LaunchRequested→Started→Stopping→Stopped as the mock
   (`applications/lifecycle_is_observable_as_events` passes for both).
6. **Cleanup** ✓ — zero orphan distros/processes (they die with the distro);
   tracked state dropped on destroy.

The contract has now survived a reference implementation, a second independent
adapter, a versioned runtime, and a real operating system. Adding Linux/macOS or
richer runtimes is henceforth implementation against a proven standard.

## Log

### #1 — `wmctrl` not in Alpine — **runtime-issue**
`apk add wmctrl` → "no such package" (Alpine packages neither main nor community).
Window discovery + focus switched to **xdotool** (packaged), which covers both:
`xdotool search` (list), `getwindowname` (title), `getactivewindow` (focus).
No contract impact — window discovery is a runtime/adapter mechanic, not contract.
Fix: drop wmctrl from `build.sh`; the readiness probe uses `xdotool
getdisplaygeometry` instead of `xdpyinfo`, removing that dependency too.

### #2 — `windows/at_most_one_focused` flaked (0 focused) — **adapter-bug**
First live `run_all`: **19/21**, zero orphans. Only failure: `at_most_one_focused`
saw 0 focused while the identically-set-up `newest_is_focused` passed — a race.
Root cause: `list_windows` queried live `xdotool getactivewindow` per call, which
is racy across separate `wsl.exe` invocations. The contract defines focus as
"newest launched, at most one" — which the reference (mock) *tracks*, it doesn't
query. Classified adapter-bug, not conformance/spec: the runtime already makes it
physically true (launcher does `windowactivate --sync` on the newest). Fix: the
adapter tracks focus = most-recently-launched live window, matching the mock's
semantics exactly. No contract change; `getactivewindow` dropped.
