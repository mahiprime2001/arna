# Spike: native Windows isolation (AppContainer) — findings

**Question:** can Windows' *own* isolation primitives give a workspace real
isolation (files/network/…) so **native** can be a strong-isolation runtime, and
Docker becomes optional rather than the answer?

## Result — filesystem isolation WORKS, natively

Probe (`appcontainer_spike.rs`, ignored test): create a Windows **AppContainer**,
then run `cmd /c type <file-in-user-profile>` twice — normally, and inside the
AppContainer.

```
control_exit=Some(0)   (a normal process READ the host file)
appcontainer_exit=Some(1)  (the AppContainer process was DENIED)
=> ISOLATION WORKS
```

Environment: Windows 11 build **26220** (past the 24H2 / 26100 line). Only
documented APIs: `CreateAppContainerProfile`, `SECURITY_CAPABILITIES` +
`PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES`, `CreateProcessW`. The contained
process **launched and ran** — it simply couldn't reach the user's files. That's
real OS-enforced filesystem isolation, no container, no VM.

## What this changes

Docker is **not required for filesystem isolation.** A **"Native Isolated"** runtime
mode is on the table: AppContainer (FS/network/registry boundary) + Job Object
(process-tree ownership, already built) + an ACL grant so the workspace's own dir is
reachable + (later) WFP for network policy. WSE becomes the *orchestration layer over
Windows security primitives* — exactly the vision.

## What this does NOT yet prove (the honest gaps)

1. **App compatibility — the real question.** This ran `cmd` (a console app).
   Whether **GUI apps run correctly inside an AppContainer** — VS Code, Chrome, with
   extensions, child processes, GPU — is unproven, and historically many Win32 apps
   break there (missing capabilities, broken IPC). This is exactly the
   **Certified / Compatible / Unsupported** tiering to establish per app.
2. **The positive case.** Granting the AppContainer SID an ACL on the *workspace*
   directory (so the app can use its own files) is untested.
3. **Network / registry.** WFP application-based network filtering and registry
   isolation are documented but unprobed here.
4. **Win32 app isolation (24H2).** The newer MSIX-packaged path is designed to make
   GUI Win32 apps work under isolation more smoothly — worth investigating as the
   route to "Certified" for the hard apps; it needs packaging.

## Recommendation

This is strong enough to justify a **"Native Isolated" runtime mode** built on
AppContainer — but gated on **app-compatibility probes**, honestly tiered:

```
VS Code  → run inside AppContainer + granted workspace dir → Certified? Compatible? Unsupported?
Chrome   → same
PowerShell / Python / Git → same
```

Next spike: launch **VS Code** (then Chrome) into an AppContainer with an ACL-granted
workspace dir, and see if it runs usably. If the target apps work, WSE gets native
strong isolation and Docker is demoted to a genuine fallback — closest yet to the
original vision. If they break, the fallback ladder (Win32 app isolation → Docker →
VM) is the honest answer, per app. **No lying** — the guarantee card tells the truth
either way.
