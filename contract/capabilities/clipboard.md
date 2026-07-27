# Capability: Clipboard

**Status: Draft** · answers to SPEC §9 · `Capability::Clipboard`

The first capability specified after the Stable pair (Applications, Windows). It
is deliberately first because it is deceptively rich — get it right and the
other capabilities reuse the pattern.

---

## 1. Intent

A workspace needs its own clipboard, isolated from the host and from every other
workspace, and it needs to move text and images **across the workspace boundary**
to and from a member's device — under policy, in each direction independently.

Copy-and-paste *inside* the workspace is not a capability concern (it never
crosses the boundary, SPEC §10.6). The Clipboard capability governs only the two
boundary-crossing transfers.

## 2. Contract

**Data model.** A clipboard payload is a MIME/content-type plus its bytes:

```
ClipboardItem { mime: String, payload: Vec<u8> }
```

Formats are **data, not capabilities** — `text/plain`, `text/html`, `image/png`,
`image/jpeg`, `application/json` all flow through this one model, so the contract
never changes when a new format appears (Windows/Linux/macOS all think in
MIME/UTI/content-type terms anyway). `text/plain` is the baseline every
implementation carries; richer types are best-effort per adapter. An adapter
that cannot place a given content-type on its native clipboard maps that to
`Internal` (a mechanical failure), not a capability or permission error.

**Interface (adapter, mechanical).** The adapter only reads/writes the
workspace's own clipboard. It never decides *who* may do so — that is the
engine's policy.

```
trait ClipboardCapability {
    fn clipboard_peek(&self,   id) -> Option<ClipboardData>   // the workspace clipboard's content
    fn clipboard_put (&mut,    id, data)                      // replace the workspace clipboard
}
```

**Interface (engine, policy).** The engine gates the two boundary transfers by
role and access right, then calls the adapter:

```
engine.clipboard_read_out(id, role) -> Option<ClipboardData>   // member copies OUT of the workspace
engine.clipboard_write_in(id, role, data)                      // member pastes INTO the workspace
```

Read-out and write-in are **separate operations gated by separate rights** and
MUST NOT be granted as one (SPEC §9.2).

## 3. State model

The workspace owns exactly one clipboard slot: `Option<ClipboardData>` (empty
until first written). It belongs to the workspace, shared by its members — not
per-member. The state lives with the workspace, so it is retained across pause
and restored across save (SPEC §9.1, §16.4) — an adapter that persists workspace
state persists the clipboard with it.

## 4. Invariants

- **I1 — Isolation.** A workspace's clipboard is not readable or writable from
  the host or any other workspace (SPEC §9.1). Two workspaces never share.
- **I2 — Direction is independent.** `read_out` requires `ClipboardRead`;
  `write_in` requires `ClipboardWrite`. Holding one never implies the other
  (SPEC §9.2).
- **I3 — Observing is not extracting.** An Observer is refused `read_out` by
  default; data extraction is not observation (SPEC §4.6.1, §9.2.1). By default
  an Observer is also refused `write_in`.
- **I4 — Owner authority.** The Owner may read and write (SPEC §4.6).
- **I5 — Auditable, not logged.** Every transfer emits an event carrying *who*
  and *which direction* — never the content (SPEC §17.1 vs. privacy).
- **I6 — Absence is typed.** Operating on a workspace that does not declare
  Clipboard fails as `CapabilityUnavailable(Clipboard)`, distinct from a
  permission refusal.

## 5. Error mapping

Only these `WseError` values may arise from a Clipboard operation:

| Situation | Error |
|---|---|
| Workspace doesn't provide Clipboard | `CapabilityUnavailable(Clipboard)` |
| Role lacks the required right (incl. Observer) | `PermissionDenied { right, role }` |
| Workspace id unknown | `NotFound` |
| A content-type the adapter can't represent natively | `Internal` |
| Adapter/platform failure | `Internal` |

`PermissionDenied` is a **visible** refusal (the clipboard's existence is not a
secret), which is why it is not the §6.5 `NotFound` used for undetectable
resources.

## 6. Conformance tests (`run_clipboard`)

Runs only for adapters that declare `Capability::Clipboard`:

- `clipboard/isolated_per_workspace` — write into one workspace, another sees
  nothing (I1).
- `clipboard/owner_roundtrips` — Owner write_in then read_out returns the same
  payload (I4).
- `clipboard/read_and_write_are_separate_rights` — a Collaborator with write but
  not read may write_in and is refused read_out (I2).
- `clipboard/observer_refused_read_out` — Observer read_out → `PermissionDenied`
  (I3).
- `clipboard/observer_refused_write_in` — Observer write_in → `PermissionDenied`
  (I3).

Plus, tested at the engine level against a no-clipboard adapter:
`CapabilityUnavailable` when the capability isn't declared (I6).

## 7. Reference implementation

The mock adapter holds an in-memory `Option<ClipboardData>` per workspace and
implements `ClipboardCapability`. It declares `Capability::Clipboard` and
therefore runs this suite via `run_all`.

## 8. Platform implementations

Later, and as consumers: the Windows/Linux/macOS adapters map their native
clipboard into `ClipboardCapability`, or declare the capability unavailable.
None of them changes this spec.

---

## Open questions (why this is Draft, not Stable)

- Multiple simultaneous representations on one clipboard write (text + html +
  image together), as real OS clipboards do — likely `Vec<ClipboardItem>`.
- Size limits and whether they are a Resource concern or a Clipboard one.

*(Resolved: formats are data, not sub-capabilities — hence the single MIME model.)*
