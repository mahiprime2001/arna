//! The first milestone, as a conformance test. Not "it runs" — it runs AND the
//! contract invariants hold: the lifecycle state machine, deny-by-default with
//! §6.5 undetectability, and the §18.3 isolation core. The mock is the
//! reference; every real adapter must pass the equivalent.

use wse_adapter_mock::MockAdapter;
use wse_common::{ApplicationDescriptor, EventKind, Persistence, WorkspaceState, WseError};
use wse_engine::{Engine, WorkspaceConfig};

fn catalog() -> Vec<ApplicationDescriptor> {
    vec![
        ApplicationDescriptor::new("browser", "Browser"),
        ApplicationDescriptor::new("editor", "Editor"),
        ApplicationDescriptor::new("terminal", "Terminal"),
    ]
}

#[test]
fn create_start_launch_list() {
    let mut engine = Engine::new(MockAdapter::new());

    let ws = engine
        .create_workspace(WorkspaceConfig::new(
            "Design review",
            Persistence::Temporary,
            catalog(),
        ))
        .unwrap();
    assert_eq!(engine.state(&ws), Some(WorkspaceState::Created));

    engine.start(&ws).unwrap();
    assert_eq!(engine.state(&ws), Some(WorkspaceState::Running));

    // The exact shape from the plan.
    let _w1 = engine.launch(&ws, "browser").unwrap();
    let _w2 = engine.launch(&ws, "editor").unwrap();
    let windows = engine.list_windows(&ws).unwrap();
    assert_eq!(windows.len(), 2);
    // Only the newest window is focused.
    assert!(windows.last().unwrap().focused);
    assert!(!windows.first().unwrap().focused);
}

#[test]
fn deny_by_default_is_undetectable() {
    // SPEC §6.5: an app that isn't in the catalog is *not found*, never *denied*.
    let mut engine = Engine::new(MockAdapter::new());
    let ws = engine
        .create_workspace(WorkspaceConfig::new(
            "ws",
            Persistence::Temporary,
            catalog(),
        ))
        .unwrap();
    engine.start(&ws).unwrap();

    match engine.launch(&ws, "photoshop") {
        Err(WseError::NotFound(_)) => {} // correct: indistinguishable from nonexistent
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn cannot_launch_before_running() {
    let mut engine = Engine::new(MockAdapter::new());
    let ws = engine
        .create_workspace(WorkspaceConfig::new(
            "ws",
            Persistence::Temporary,
            catalog(),
        ))
        .unwrap();
    // Not started yet.
    assert!(matches!(
        engine.launch(&ws, "browser"),
        Err(WseError::InvalidState { .. })
    ));
}

#[test]
fn state_machine_rejects_illegal_transitions() {
    // SPEC §5.2: e.g. you cannot stop-to-Saved a workspace that never started.
    let mut engine = Engine::new(MockAdapter::new());
    let ws = engine
        .create_workspace(WorkspaceConfig::new(
            "ws",
            Persistence::Temporary,
            catalog(),
        ))
        .unwrap();
    // Created -> Saved is not a permitted transition.
    assert!(matches!(
        engine.stop(&ws),
        Err(WseError::InvalidTransition { .. })
    ));
}

#[test]
fn refuses_unsealed_workspace() {
    // SPEC §18.3: if the adapter can't prove isolation, the engine refuses to
    // run it. A one-off adapter that reports sealed=false must be rejected.
    use wse_common::{CapabilitySet, Result, WorkspaceId};
    use wse_contract::{IsolationAttestation, WorkspaceAdapter, WorkspaceDef};

    // A minimal adapter: lifecycle + isolation only, no capabilities.
    #[derive(Default)]
    struct LeakyAdapter;
    impl WorkspaceAdapter for LeakyAdapter {
        fn capabilities(&self) -> CapabilitySet {
            CapabilitySet::none()
        }
        fn create(&mut self, _def: &WorkspaceDef) -> Result<()> {
            Ok(())
        }
        fn start(&mut self, _id: &WorkspaceId) -> Result<IsolationAttestation> {
            Ok(IsolationAttestation {
                model: wse_common::IsolationModel::SealedVm,
                isolated: false,
                details: vec!["host drive visible at /mnt/c".into()],
            })
        }
        fn stop(&mut self, _id: &WorkspaceId) -> Result<()> {
            Ok(())
        }
        fn destroy(&mut self, _id: &WorkspaceId) -> Result<()> {
            Ok(())
        }
    }

    let mut engine = Engine::new(LeakyAdapter);
    let ws = engine
        .create_workspace(WorkspaceConfig::new(
            "ws",
            Persistence::Temporary,
            catalog(),
        ))
        .unwrap();
    assert!(matches!(
        engine.start(&ws),
        Err(WseError::IsolationRejected { .. })
    ));
    // And it must NOT be left in a running state.
    assert_eq!(engine.state(&ws), Some(WorkspaceState::Created));
}

#[test]
fn clipboard_op_on_non_declaring_adapter_is_unavailable() {
    // clipboard spec I6 — a capability op on a workspace that does not declare
    // the capability fails as CapabilityUnavailable, not a permission refusal.
    use wse_common::{Capability, CapabilitySet, ClipboardItem, Result, Role, WorkspaceId};
    use wse_contract::{IsolationAttestation, WorkspaceAdapter, WorkspaceDef};

    // A minimal adapter that declares NO optional capabilities.
    #[derive(Default)]
    struct NoClipboardAdapter;
    impl WorkspaceAdapter for NoClipboardAdapter {
        fn capabilities(&self) -> CapabilitySet {
            CapabilitySet::none()
        }
        fn create(&mut self, _def: &WorkspaceDef) -> Result<()> {
            Ok(())
        }
        fn start(&mut self, _id: &WorkspaceId) -> Result<IsolationAttestation> {
            Ok(IsolationAttestation {
                model: wse_common::IsolationModel::SealedVm,
                isolated: true,
                details: vec![],
            })
        }
        fn stop(&mut self, _id: &WorkspaceId) -> Result<()> {
            Ok(())
        }
        fn destroy(&mut self, _id: &WorkspaceId) -> Result<()> {
            Ok(())
        }
        // No capability hooks -> all None.
    }

    let mut engine = Engine::new(NoClipboardAdapter);
    let ws = engine
        .create_workspace(WorkspaceConfig::new(
            "ws",
            Persistence::Temporary,
            catalog(),
        ))
        .unwrap();
    // Neither Clipboard nor Storage is declared by NoClipboardAdapter.
    assert!(matches!(
        engine.clipboard_read_out(&ws, Role::Owner),
        Err(WseError::CapabilityUnavailable(Capability::Clipboard))
    ));
    assert!(matches!(
        engine.clipboard_write_in(&ws, Role::Owner, ClipboardItem::text("x")),
        Err(WseError::CapabilityUnavailable(Capability::Clipboard))
    ));
    assert!(matches!(
        engine.storage_create(&ws, Role::Owner, "x", wse_common::ResourceKind::Blob),
        Err(WseError::CapabilityUnavailable(Capability::Storage))
    ));
    assert!(matches!(
        engine.device_enumerate(&ws),
        Err(WseError::CapabilityUnavailable(Capability::Devices))
    ));
}

#[test]
fn events_are_observable() {
    let mut engine = Engine::new(MockAdapter::new());
    let ws = engine
        .create_workspace(WorkspaceConfig::new(
            "ws",
            Persistence::Temporary,
            catalog(),
        ))
        .unwrap();
    engine.start(&ws).unwrap();
    engine.launch(&ws, "browser").unwrap();

    let events = engine.events_for(&ws);
    // First event is the creation; some later event is the app start.
    assert!(matches!(events.first().unwrap().kind, EventKind::WorkspaceCreated));
    assert!(events
        .iter()
        .any(|e| matches!(e.kind, EventKind::ApplicationStarted { .. })));
    // Per-workspace seq is a strictly increasing ordering authority.
    for pair in events.windows(2) {
        assert!(pair[1].seq > pair[0].seq);
    }
}
