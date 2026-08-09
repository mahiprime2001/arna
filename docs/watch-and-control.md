# Watch & Control — remote workspace sessions

The second half of collaboration. Join gave **membership at a role**; Watch &
Control turns a role into **enforced permissions over a live workspace surface**.

> Product/session layer — **no engine or WRM change**. This wraps the runtime's
> surface; it does not touch the frozen contract, projector, or execution model.

## The model: `RemoteSession`

A host shares a workspace **surface** (video) with guests; **input** flows back
over a separate channel the host **gates by role**. The guest never receives the
workspace URL — only a stream and a (possibly rejected) input channel.

```
Host workspace surface ──capture──▶ WebRTC video ──▶ Guest (watches)
Guest input ──data channel──▶ Host GATE ──(only if Controller)──▶ inject into surface
```

The **host is the single enforcement point.** This is why it works the same for
Docker (capture the code-server window) and native (capture the workspace desktop).

## The enforcement rule (non-negotiable)

Role controls capabilities **at the enforcement point**, never by hiding a button:

| Role | video/data receive | input send |
|------|--------------------|-----------|
| **Viewer** | allowed | **rejected at the host** |
| **Controller** | allowed | allowed |

A Viewer's input events reach the host and are **dropped there**. Granting control
promotes exactly one guest; there is **at most one Controller at a time**.

## Host controls

- **Grant control** → promote a guest to Controller (demotes the previous one).
- **Revoke control** → the guest immediately loses input (back to Viewer).
- **Disconnect** → the guest is removed: input rejected *and* the surface stream
  is torn down. Access ends immediately.

## Acceptance test

```
Invite Viewer → guest joins → guest SEES the workspace → guest CANNOT interact
→ host grants Controller → guest CAN control
→ host revokes → guest immediately loses input
→ host disconnects → guest loses the workspace surface
```

## Build order

1. **Enforcement core** (this first): the `RemoteSession` state machine + the input
   gate + host controls, host-side, unit-tested against the acceptance invariants.
   `remote.rs` in the app backend, with real OS input injection (Windows SendInput)
   behind the gate. *(Docker surface first.)*
2. **Transport**: WebRTC video (host `getDisplayMedia` of the workspace surface) +
   an input data channel, signalled through the existing relay. Guest renders the
   video and forwards pointer/key events.
3. **Native**: the same session against a native workspace desktop surface.

Steps 2–3 require two machines to exercise; step 1 is unit-testable in isolation
and is where the enforcement guarantee actually lives.
