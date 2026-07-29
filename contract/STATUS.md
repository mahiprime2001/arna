# Conformance Status

The engineering dashboard: what each adapter implements, and its conformance
against the contract. Not marketing — this is how regressions and gaps are seen
at a glance. (Hand-maintained today; generated from `run_all` results later.)

Legend: ✓ passes its suite · ⏳ declared next, not yet · – not started ·
`n/a` capability not applicable.

## Adapters × capabilities

| Capability | Suite checks | Mock | Windows (WSL2) | Linux | macOS |
|------------|:---:|:---:|:---:|:---:|:---:|
| **Core** (lifecycle, isolation, events, identity) | 9 | ✓ | ✓ (live) | – | – |
| Applications | 4 | ✓ | ⏳ | – | – |
| Windows | 2 | ✓ | ⏳ | – | – |
| Clipboard | 5 | ✓ | ⏳ | – | – |
| Storage | 8 | ✓ | ⏳ | – | – |
| Devices | 9 | ✓ | ⏳ | – | – |
| **run_all total** | **37** | **37/37** | **9/9** | – | – |

- **Mock** — the reference implementation; declares everything; 37/37.
- **Windows** — the first real platform adapter; declares only Core today
  (minimal + truthful), so `run_all` == the core suite, passing 9/9 live
  against real WSL2 and leaving zero orphan distros. Capabilities are turned on
  by shrinking the `CapabilityUnavailable` surface, in order:
  Applications → Windows → Clipboard → Storage → Devices.
- **Linux / macOS** — not started; each will be one crate implementing the same
  contract and passing this same suite.

## Contract maturity (per capability)

| Capability | Status | Spec |
|------------|--------|------|
| Applications, Windows | Stable | in-engine + core conformance |
| Clipboard, Storage, Devices | Draft | [capabilities/](capabilities/) |
| Network, Audio, Camera | Planned | — |

## Milestones

- ✓ Contract v0.1 · reference engine · conformance suite · mock adapter.
- ✓ **First independent implementation** (Windows) conforms without any contract
  change — the contract is now a *standard*, not one implementation's design.
- ✓ Conformance suite is **repeatable** (self-cleaning against real state).
- ⏳ Windows grows capability-by-capability.
- … then Linux/macOS adapters, then distributed capabilities (Network,
  Collaboration).
