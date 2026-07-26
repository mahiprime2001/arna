# Arna — Roadmap

A plain-language map of what's built and what's next. Skim the checkboxes.

**What Arna is:** a place you lend part of your computer to a friend. They get their
own sealed screen and apps; you keep using your real machine, and they can never
see your screen, files, or clipboard.

Legend: ✅ done · 🔨 building now · ⬜ not started · 💤 parked for later

---

## The big picture

Arna is two things being built in parallel:

1. **The social app** — accounts, friends, chat, calls. *(mostly done)*
2. **The workspace** — the sealed computer-in-a-computer. *(just starting)*

We built the social app first so there's something real to use while the harder
workspace layer gets built underneath it.

---

## 1 · Foundations ✅

- ✅ **The rulebook** (`docs/SPEC.md`) — frozen. Defines what a workspace *is* and the
  promises it must keep (isolation, roles, permissions), with zero mention of any
  specific technology. This is the constitution; everything answers to it.
- ✅ **The decisions** (`docs/adr/`) — 7 records of the big calls and why.
- ✅ **The blueprint** (`docs/ARCHITECTURE.md`) — how the pieces fit, and the build order.

## 2 · The social app

### Accounts & friends ✅
- ✅ Sign up / log in (Go backend + SQLite)
- ✅ Real friend requests — send, accept, decline, cancel, remove
- ✅ Online / offline presence
- ⬜ Rooms — when a friend's online, jump into a shared space *(waits on workspaces)*

### Chat ✅
- ✅ End-to-end encrypted (the server only ever sees scrambled text)
- ✅ Photos, files, voice messages
- ✅ Delivered / read receipts
- ✅ **Telegram look** — exact colours, light + dark
- ✅ Disappearing messages (self-destruct timer)
- ✅ Reply, forward, pin
- ✅ Edit, delete for everyone
- ✅ Emoji reactions + composer emoji picker
- ✅ Typing indicator, in-chat search, mute, day separators

### Calls ✅
- ✅ Real peer-to-peer audio & video (WebRTC)
- ✅ **WhatsApp look** for both call types
- ✅ Works with no mic/camera (receive-only fallback)
- ⬜ **TURN server** — so calls connect across *different* networks, not just same wifi
- 💤 Group calls

### Privacy model ✅
- ✅ Messages live on your devices, never stored on the server
- ✅ Calls go device-to-device; the server only helps them find each other

## 3 · The workspace  🔨 ← we are here

The sealed computer-in-a-computer. **Same sealed room in every version — the plan is
to move into the pre-furnished one now, and swap in our own furniture later, without
moving house.**

### Design ✅
- ✅ Workspace rules in code — states, roles, permissions (from the rulebook)
- ✅ **Create-workspace screen** — pick apps, shared folders, guest permissions, limits,
  internet on/off. *(This saves your choices; it can't run anything yet.)*

### Stage A — furnished apartment (WSL2) 🔨
> Use the sealed room Windows already ships. Fastest path to a workspace that actually
> runs. This is what we're building now.

- 🔨 **Host check** — at startup, can this machine make a sealed room at all? If not,
  Arna refuses rather than pretending. *(first piece, starting now)*
- ⬜ Adapter that creates/starts/pauses/stops a real workspace via WSL2
- ⬜ Lock the two unsafe doors (`interop`, `automount`) or refuse to start
- ⬜ Launch an app inside it (VS Code / Chrome / terminal)
- ⬜ Stream its screen to the browser
- ⬜ Send keyboard & mouse into it (and *only* it)
- ⬜ Shared-folder grants actually mount
- ⬜ Enforce roles — observers can watch but never touch

### Stage B — our own furniture (bundled image) 💤
> Same sealed room, but we bring our own slimmed-down Linux packed inside Arna. No
> `/mnt/c` door to lock because we never build one. Faster boot, less lag. Months of
> work — starts once Stage A proves the whole pipeline.

- 💤 Our own minimal guest image, shipped in the app
- 💤 Create the VM directly (no WSL, no install, no admin) via the Host Compute Service
- 💤 Capture-friendly compositor — screen → encoder with less delay
- 💤 One warm VM, many lightweight workspaces inside it (saves RAM on modest machines)

### Later adapters 💤
- 💤 Linux hosts — native, no VM needed
- 💤 macOS hosts — Apple Virtualization

## 4 · Getting it to people

- ✅ Run it across devices via a tunnel (Cloudflare / ngrok) — real HTTPS, no setup
- ✅ One-command server (app + API + chat on a single port)
- ✅ Docker + auto-HTTPS deploy files (for a always-on server)
- ⬜ Deploy the new app to the VPS so `arna.ifleon.com` serves it
- 💤 **Mobile layout** — the app works on desktop; phone screens need real work
- 💤 Stable public URL / own domain

---

## Right now

**Building:** Stage A, first piece — the **host check** that tells Arna whether this
machine can run a workspace, and refuses honestly if it can't.

**Your machine (for reference):** i5-9400F · 6 cores · 16 GB · hypervisor already
running · VirtualMachinePlatform on · WSL2 with Ubuntu present. Good to go for Stage A.

_Last updated: 2026-07-26_
