# Arna — App sharing & independent control (plan / notes)

> Plain-language plan for a future feature: sharing **one app** (or a whole
> **extra desktop**) so a remote person can **control it independently**, while
> the owner keeps using their real computer. Written so anyone can follow it; the
> "when we build it" section has the technical names. Backed by the deep-research
> findings summarised at the bottom.

## The idea in one line

Most remote tools (TeamViewer, AnyDesk, Parsec) share your **whole screen** and
hand over your **one** mouse — so only one person can work at a time. We want a
mode where you share **just one app** (or a separate desktop), the other person
**controls only that**, and **you keep working** on your own stuff at the same time.

## The core problem (why this is hard)

A normal computer has **one mouse and one keyboard** that everything shares. When
someone controls your machine remotely, they grab *that one* mouse — so you can't
both work at once (you take turns). To have two people working independently, the
shared app needs **its own separate set of controls**.

- **Linux** was built so an app and its screen can live in separate places — so
  "run an app here, control it there, independently" is **natural** (see research).
- **Windows & macOS** weld the app to your one logged-in desktop — so the only
  reliable way to get separate controls is to run the shared app inside a
  **"bubble"**: a sandbox / small virtual machine with its **own** mouse + keyboard.

## The three modes

| Mode | What it is | Sandbox? | Status |
|---|---|---|---|
| **1. Whole-screen share** | They see/drive your real desktop (what we have today). One mouse — you step back while they drive. | No | ✅ built |
| **2. Share an app** | Pick from a **curated list** of apps → owner approves → the app opens in a **bubble** (hidden desktop) → they control it, owner keeps working. | Yes (hidden desktop) | ✅ **built** (Windows; classic apps; with consent popup) |
| **3. Extended screen** | A whole **extra desktop** in a bubble → they work there, owner works on the real screen. | Yes | 🔜 plan (same engine as #2, bigger) |

Modes 2 and 3 are the **same machinery** — a bubble with its own controls — just a
different size (one app vs a whole desktop). The sandbox UI/options only appear in
modes 2 and 3; whole-screen share (mode 1) stays simple and unchanged.

## Mode 2 — "Share an app" (the sweet spot to build first)

**Why a curated list is the clever part.** "Share *any* app independently" is hard
because some apps misbehave in a bubble (games, odd custom software). If **we** pick
the list — Chrome, a file manager, an office app, etc., all **tested to run well in
a sandbox** — we remove the hardest part and only ever offer things that work. This
is what makes an "impossible" feature actually shippable.

**The flow:**
1. The remote person picks an app from the offered list (e.g. "Chrome").
2. The owner gets a consent popup: **"Allow them to open Chrome in a safe sandbox?"**
   (reuses our existing Accept/Decline + code consent).
3. On accept, Arna spins up a bubble, launches the app inside it, streams **just
   that bubble** to the remote person, and routes the remote person's mouse/keyboard
   **into the bubble** (not the owner's real desktop).
4. The owner keeps using their real computer normally. No collision.
5. Ending the session closes the bubble.

**Two honest notes:**
- The bubble's app is a **fresh copy**, not the owner's currently-open one — its own
  login, its own data. For this use case that's a **plus** (privacy: the remote
  person can't see the owner's accounts/files).
- A bubble uses real CPU/RAM. **Lightest on Linux**, heavier on Windows/macOS.

## Mode 3 — "Extended screen"

Same as mode 2, but the bubble is a **whole empty desktop** the remote person works
in (they can open several things). Important gotcha: a plain "second monitor" alone
does **not** give a second mouse — it's just more space everyone still shares. For
*independent* work, the extra desktop must be a **bubble with its own controls**.
So this is the same engine as mode 2, sized up to a full desktop.

## How it reuses what we already built

Our engine already does **capture a screen → encode → stream over WebRTC** and
**inject mouse/keyboard**. The bubble approach just **aims that same engine at the
bubble's screen** instead of the whole desktop, and routes input into the bubble.
So we're extending the existing pipeline, not starting over.

## Per-platform feasibility

| | Share one app (mode 2) | Extended desktop (mode 3) | Independent control |
|---|---|---|---|
| **Linux** | Easy — lightweight (virtual display / container; `Xpra` already does app-window forwarding) | Easy-ish (virtual display + own seat) | ✅ natural |
| **Windows** | Workable but heavier — needs a managed sandbox/VM + virtual display + input routing | Heavier (virtual desktop in a bubble) | ⚠️ only inside the bubble |
| **macOS** | Hardest — heavy VM; no native input injection into a window | Hardest | ⚠️ only inside a VM |

**Recommended order:** Linux first (cheapest, and it's our open-source/self-host
crowd), then the heavier Windows version, then macOS if it's worth it.

## Rough effort (T-shirt sizes)

- **Mode 2 on Linux:** Medium — create a virtual display / container, launch the
  chosen app, point capture + input at it, plus the app-list UI + consent.
- **Mode 2 on Windows:** Large — manage a sandbox/VM, a virtual display driver,
  reliable input routing, and bundle/guide the setup.
- **Mode 2 on macOS:** Large / uncertain — VM-based; licensing + performance questions.
- **Mode 3 (extended screen):** Incremental on top of mode 2 (same bubble engine,
  bigger surface).

## Open questions to resolve before building

- Exactly which "bubble" tech per platform (lightweight container vs full VM), and
  how to bind **isolated input** (a separate cursor) to the bubble so the owner's
  real desktop is untouched.
- The **starter app list** and how we test/package each app for the sandbox.
- Wayland (the future of Linux display) is more locked-down than X11 — check
  `waypipe` maturity and the PipeWire screen-capture rules.
- Licensing if we lean on existing streaming tech (e.g. Sunshine is GPL-3.0).

## ✅ Validated: a VM-less Windows bubble (hidden desktop) — works on Home

We proved a **bubble without a VM or Windows Sandbox** on Windows 11 **Home**
(where Sandbox/Hyper-V aren't even offered). PoC: `agent/examples/bubble_poc.rs`
(`cargo run -p arna-agent --example bubble_poc -- charmap.exe <out_dir>`).

The trick is the Windows **desktop object** (`CreateDesktopW`): a second, hidden
desktop within the same session. The owner's real desktop keeps receiving the
physical mouse/keyboard; the bubble lives off to the side, unseen.

Proven end-to-end on this machine:
1. **Isolated run** — launch an app on the hidden desktop with `CreateProcessW`
   + `STARTUPINFOW.lpDesktop`. It never appears on the owner's desktop.
2. **Capture** — `PrintWindow(hwnd, …, PW_RENDERFULLCONTENT)` returns the app's
   real pixels (94% non-black for Character Map — a clean shot, not a black
   frame). Feeds straight into our existing H.264 pipeline.
3. **Input** — `PostMessage(WM_CHAR / WM_MOUSE*)` to the window (or its child
   Edit control) drives it with **no global cursor** — so it doesn't fight the
   owner's mouse. Typed text landed in the field; the Copy button enabled.

**Honest limits (this is why the app list is curated):**
- Great for **classic Win32 apps** (Explorer windows, Office classics, utilities).
- **Unreliable for Chromium/Electron/DirectX/games** — `PrintWindow` can come
  back black and `PostMessage` doesn't reach their input. Those need the heavier
  VM/Sunshine path. So the curated list ships only apps tested on this technique.
- It's **isolation-from-view + a separate input channel, not a security sandbox**
  (the app runs as the same user). Real security isolation still wants a VM.

So Mode 2 ("share one app") is **buildable today on stock Windows** for curated
classic apps. Remaining work to ship it: drive `PrintWindow` in a ~30fps capture
loop into the video track, translate remote pointer/keys → window messages,
add the curated app picker + consent, and clean up the desktop/app on exit.

## When we build it — technical building blocks (from research)

- **Linux:** `Xpra` ("screen for X11") forwards individual app windows as native
  windows, with attach/detach — closest to mode 2 out of the box. `Xvfb` = headless
  virtual screen (the bubble for mode 3). Multi-seat via upstream Xorg (the old
  `multi-seat-xephyr` is obsolete; its patches landed in Xorg server 1.19+).
- **Windows:** RDS **RemoteApp** is genuine per-app remoting but it's a Server/RDS
  feature (client SKUs are single-session; the `rdpwrap` hack is fragile). Background
  per-window input injection (`PostMessage`/`SendMessage`) is **unreliable** for
  modern Chromium/Electron/DirectX apps — so prefer the bubble + isolated input.
- **macOS:** `ScreenCaptureKit` can capture a single window but is **capture-only**
  (no input injection) — independent control needs a VM.
- **Cross-platform fallback:** the cloud-gaming model — **Sunshine** (GPL-3.0,
  self-hosted, hardware-encoded, runs on Win/Linux/macOS) + **Moonlight**, i.e. run
  the app/desktop in an isolated session with a virtual display and stream it.
  `moonlight-web-stream` (Rust + WebRTC bridge) shows this fits a Tauri/Rust + WebRTC
  stack like ours.

> The fundamental reason Linux is easy: its display system (X11) is
> **network-transparent client-server** — apps and the screen were always allowed to
> live on different machines. Windows/macOS tie the GUI to one local session, which
> is why they need the bubble. (Deep-research verified, 2026.)
