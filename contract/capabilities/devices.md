# Capability: Devices (External Resources)

**Status: Draft** · answers to SPEC §7.2/§7.3, §12 · `Capability::Devices`

Not "cameras and microphones". The capability represents:

> **host-provided resources that are external to the workspace and may be made
> available to it.**

That framing covers camera, microphone, speaker, display, printer, USB, GPU,
Bluetooth, NFC, and any future sensor — so the contract never grows because a new
device class appears. **Device classes are data, not capabilities.**

Streaming is out of scope: this capability answers *what devices exist, what
their properties are, and how they are made available*. It does **not** move
frames, samples, audio, or video — that belongs to a future media/stream
contract.

---

## 1. Intent

Represent external host resources and mediate making them available to a
workspace, under policy, without ever leaking platform terminology.

## 2. Contract

**Discovery, authorization, and usage are separate operations** — the most
important boundary in this capability:

```
enumerate(ws) -> [DeviceDescriptor]     // discovery: "what is available?"
request(ws, role, device) -> DeviceHandle  // authorization: "may I use it?"
release(ws, role, device) -> bool          // usage lifecycle
state(ws) -> CapabilityState               // Available | Degraded | ...
```

Host-side availability (the discovery *source*):

```
attach(ws, class, name) -> DeviceDescriptor   // host makes a device available (§12.1/§12.4)
detach(ws, device) -> bool
```

## 3. Data model

Three separate concepts — a descriptor is **not** a permission, **not** a
session, **not** a handle:

```
DeviceId                                   // stable, immutable identity
DeviceClass { Camera, Microphone, Speaker, Display, Printer, Usb, Gpu, Bluetooth, Nfc }
DeviceDescriptor { id, class, name, metadata }   // immutable description
DeviceHandle { device }                    // the result of a granted request
```

The descriptor/handle split is a **reusable abstraction** — later capabilities
reuse it, just as Storage's Resource pattern is reused.

## 4. State model

The capability itself has a **contract state** (`CapabilityState`) — this is
where recorded capability-states become real semantics:

```
Unavailable | Available | ReadOnly | Degraded | Offline
```

These describe the **contract**, never the platform. A crashed driver maps to
`Degraded`; an unplugged device to `Unavailable`; a policy block to `ReadOnly` or
`Unavailable`. The adapter translates platform reality into these; the contract
never leaks platform terminology.

A device is either available (enumerable, requestable) or not present. A granted
request yields a handle; release ends it.

## 5. Invariants

- **I1 — Nothing by default.** A workspace has no devices until the host makes
  one available (SPEC §12.1).
- **I2 — Non-available is invisible.** A workspace cannot enumerate or detect a
  device not made available to it; requesting an unknown device is `NotFound`,
  never "denied" (§12.1, §6.5).
- **I3 — Host machine devices are never reachable.** The Host Machine's own
  camera/microphone MUST NEVER be made available, by anyone, including the Host
  (§7.3). This is not a grant; an adapter never surfaces them.
- **I4 — Discovery ≠ authorization ≠ usage.** Enumerating a device grants no
  right to use it; a `DeviceHandle` comes only from a `request` the engine
  authorized.
- **I5 — Stable, immutable descriptors.** A `DeviceId` names one device for its
  life; a `DeviceDescriptor` is immutable (identity rule).
- **I6 — Authorization is policy.** `request`/`release` require the `UseDevice`
  right — Owner holds it, Observer never (touches nothing, §4.6.1), Collaborator
  only if granted (§12.4 consent, §4.6.2). Enumeration and state are ungated
  reads of already-available devices.
- **I7 — Per-workspace isolation.** A device made available to one workspace is
  invisible to another.
- **I8 — Auditable via events.** attach/detach/request/release and capability
  state changes all flow through the core event envelope — no special pipeline.

## 6. Error mapping

| Situation | Error |
|---|---|
| Workspace doesn't provide Devices | `CapabilityUnavailable(Devices)` |
| Unknown/non-available device | `NotFound` |
| Role lacks `UseDevice` | `PermissionDenied { UseDevice, role }` |
| Unknown workspace | `NotFound` |
| Adapter/platform failure | `Internal` |

## 7. Conformance tests (`run_devices`)

- `devices/none_by_default` — a fresh workspace enumerates nothing (I1).
- `devices/enumerate_lists_available` — after attach, the device appears (I5).
- `devices/non_available_is_not_found` — request an unknown id → `NotFound` (I2).
- `devices/observer_refused_request` — Observer request → `PermissionDenied` (I6).
- `devices/request_then_release` — Owner request yields a handle; release true (I4).
- `devices/isolated_per_workspace` — a device in one workspace is invisible in
  another (I7).
- `devices/state_reflects_availability` — no devices → `Unavailable`; after attach
  → `Available` (capability state).
- `devices/state_change_emits_event` — attaching the first device emits
  `CapabilityStateChanged` through the core envelope (I8).

Plus engine-level: a device op on a non-declaring adapter is
`CapabilityUnavailable`.

## 8. Reference implementation

The mock holds a per-workspace `DeviceId -> DeviceDescriptor` map and a set of
open handles; state derives from availability (empty → Unavailable, else
Available). It declares `Capability::Devices` and runs this suite via `run_all`.

---

## Deliberately out of scope (future media/stream contract)

camera frames · microphone samples · audio/video transport · rendering ·
per-device fine-grained state · hot-plug beyond attach/detach.
