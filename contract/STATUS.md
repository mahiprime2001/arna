# Conformance Status

The engineering dashboard: what each adapter implements, and its conformance
against the contract. Not marketing — this is how regressions and gaps are seen
at a glance. (Hand-maintained today; generated from `run_all` results later.)

Legend: ✓ passes its suite · ⏳ declared next, not yet · – not started ·
`n/a` capability not applicable.

## Adapters × capabilities

| Capability | Suite checks | Mock | Windows (WSL2) | Linux | macOS |
|------------|:---:|:---:|:---:|:---:|:---:|
| **Core** (lifecycle, isolation, events, identity, **runtime**) | 12 | ✓ | ✓ (live) | – | – |
| Applications (lifecycle: descriptor→instance) | 7 | ✓ | ✓ (live) | – | – |
| Windows | 2 | ✓ | ✓ (live) | – | – |
| Clipboard | 5 | ✓ | ⏳ | – | – |
| Storage | 8 | ✓ | ⏳ | – | – |
| Devices | 9 | ✓ | ⏳ | – | – |
| **run_all total** | **43** | **43/43** | **21/21** | – | – |

- **Mock** — the reference implementation; declares everything; 43/43. Its runtime
  is an in-memory environment (`mock v1.0.0`) providing the full set.
- **Windows** — the first real platform adapter; **21/21 live** against real WSL2
  (Core + Applications + Windows), launching real applications, zero orphan
  distros. It runs the **[`wse-linux-x11` v1.0.0](../runtimes/wse-linux-x11/README.md)**
  runtime (Xvfb + openbox + xterm + xdotool + launcher), which *provides*
  Applications + Windows inside the workspace while the adapter *bridges* them —
  effective = adapter ∩ runtime. Clipboard/Storage/Devices are next: each is a
  runtime capability the image will ship plus an adapter bridge.
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
