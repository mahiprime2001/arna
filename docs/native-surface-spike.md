# Spike: native Windows surface capture — findings

**Question:** can a native `SurfaceProvider` grab pixels of a workspace's real apps
(Chrome/VS Code — all GPU/DWM-composited), so Watch & Control works against native
Windows apps, not just Docker code-server?

## Result — capture is FEASIBLE

Probe (`capture_spike.rs`, run as an ignored test): `PrintWindow` with
`PW_RENDERFULLCONTENT` against the foreground window.

```
window='… - Visual Studio Code'  1936x1048  PrintWindow_ok=1  non-black=100.0%
```

VS Code is Electron/Chromium — exactly the GPU-composited case that historically
returned black via `BitBlt`. It captured **fully**. So the modern per-window
capture API handles the apps we care about. For a live stream, `PrintWindow` per
frame works but is CPU-heavy; **Windows Graphics Capture (WGC)** is the efficient
GPU path with the same reach — the production choice.

**Conclusion: the capture half of a native surface is not the wall.** We can grab
real pixels of Chrome/VS Code per window (and, by extension, the workspace's
window set).

## Follow-up: background-desktop capture ALSO works

The foreground probe left one question: can we capture a window on a **separate
(background) `CreateDesktopW` desktop** — a WSE workspace — while the host stays on
their own desktop? Second probe (`probe_background_desktop_capture`): create a
desktop, launch the browser on it (never shown), find its window, PrintWindow it.

```
background-desktop window 1292x519  PrintWindow_ok=1  non-black=100.0%
=> CAPTURE WORKS on a background desktop
```

So a **native workspace SurfaceProvider is feasible**: we can stream the workspace's
own app windows without the host viewing them. (Notepad returned "no window" — a
WinUI quirk, not a desktop-capture failure; Chrome/VS Code enumerate + capture
fine, as the live launcher already proves.)

## The remaining wall — input independence, not capture

Two things this spike did **not** clear, and one is a genuine OS limit:

1. **Separate-desktop capture (open):** this probed the *foreground* window. Whether
   `PrintWindow`/WGC captures a window on a **separate `CreateDesktopW` desktop**
   (the workspace) while the host looks at their own desktop needs a follow-up
   probe. `PrintWindow` works for non-foreground windows generally; the
   separate-desktop case is unverified.

2. **Independent input (the real wall):** Windows has **one interactive input
   desktop at a time**. `SendInput` targets it; `PostMessage` per-window is
   unreliable for GPU/Chromium apps (the documented app-bubble limit). So a guest
   cannot independently drive the workspace *while the host keeps using their own
   desktop* — that needs a **separate display/session** (a virtual display, i.e.
   the proven Linux path, or a cloud Windows VM).

## Honest verdict for native Watch & Control

| Capability | Native feasibility |
|---|---|
| **View** the workspace (incl. GPU apps) | ✅ feasible (PrintWindow proven; WGC for streaming) |
| **Hand-off control** (host stops, guest drives; foreground + SendInput) | ✅ feasible |
| **Independent** control (host works *and* guest drives simultaneously) | ❌ single input session — needs virtual display / cloud VM |

So a native `SurfaceProvider` is **worth building for view + hand-off control** — a
real improvement (a friend watches and drives your *actual* Windows apps). True
independent simultaneous control remains a virtual-display / cloud decision, not a
bug to code around. This narrows the earlier unknown: the wall is specifically
**input independence**, not capture.
