//! The first milestone, as a conformance test. Not "it runs" — it runs AND the
//! contract invariants hold: the lifecycle state machine, deny-by-default with
//! §6.5 undetectability, and the §18.3 isolation core. The mock is the
//! reference; every real adapter must pass the equivalent.

use wse_adapter_mock::MockAdapter;
use wse_common::{Persistence, WorkspaceState, WseError, AppSpec};
use wse_engine::{Engine, Event, WorkspaceConfig};

fn catalog() -> Vec<AppSpec> {
    vec![
        AppSpec::new("browser", "Browser"),
        AppSpec::new("editor", "Editor"),
        AppSpec::new("terminal", "Terminal"),
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
    use wse_common::{CapabilitySet, Result, Window, WorkspaceId};
    use wse_contract::{IsolationAttestation, WorkspaceAdapter, WorkspaceDef};

    #[derive(Default)]
    struct LeakyAdapter;
    impl WorkspaceAdapter for LeakyAdapter {
        fn capabilities(&self) -> CapabilitySet {
            CapabilitySet::default()
        }
        fn create(&mut self, _def: &WorkspaceDef) -> Result<()> {
            Ok(())
        }
        fn start(&mut self, _id: &WorkspaceId) -> Result<IsolationAttestation> {
            Ok(IsolationAttestation {
                sealed: false,
                details: vec!["host drive visible at /mnt/c".into()],
            })
        }
        fn stop(&mut self, _id: &WorkspaceId) -> Result<()> {
            Ok(())
        }
        fn destroy(&mut self, _id: &WorkspaceId) -> Result<()> {
            Ok(())
        }
        fn launch(&mut self, _id: &WorkspaceId, _app: &AppSpec) -> Result<Window> {
            unreachable!()
        }
        fn list_windows(&self, _id: &WorkspaceId) -> Result<Vec<Window>> {
            Ok(vec![])
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
    use wse_common::{
        Capability, CapabilitySet, ClipboardData, Result, Role, Window, WorkspaceId,
    };
    use wse_contract::{IsolationAttestation, WorkspaceAdapter, WorkspaceDef};

    #[derive(Default)]
    struct NoClipboardAdapter;
    impl WorkspaceAdapter for NoClipboardAdapter {
        fn capabilities(&self) -> CapabilitySet {
            // Declares Applications/Windows but NOT Clipboard.
            CapabilitySet::none()
                .with(Capability::Applications)
                .with(Capability::Windows)
        }
        fn create(&mut self, _def: &WorkspaceDef) -> Result<()> {
            Ok(())
        }
        fn start(&mut self, _id: &WorkspaceId) -> Result<IsolationAttestation> {
            Ok(IsolationAttestation {
                sealed: true,
                details: vec![],
            })
        }
        fn stop(&mut self, _id: &WorkspaceId) -> Result<()> {
            Ok(())
        }
        fn destroy(&mut self, _id: &WorkspaceId) -> Result<()> {
            Ok(())
        }
        fn launch(&mut self, _id: &WorkspaceId, _app: &AppSpec) -> Result<Window> {
            unreachable!()
        }
        fn list_windows(&self, _id: &WorkspaceId) -> Result<Vec<Window>> {
            Ok(vec![])
        }
        // No clipboard() override -> None.
    }

    let mut engine = Engine::new(NoClipboardAdapter);
    let ws = engine
        .create_workspace(WorkspaceConfig::new(
            "ws",
            Persistence::Temporary,
            catalog(),
        ))
        .unwrap();
    assert!(matches!(
        engine.clipboard_read_out(&ws, Role::Owner),
        Err(WseError::CapabilityUnavailable(Capability::Clipboard))
    ));
    assert!(matches!(
        engine.clipboard_write_in(&ws, Role::Owner, ClipboardData::text("x")),
        Err(WseError::CapabilityUnavailable(Capability::Clipboard))
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

    let events = engine.events();
    assert!(matches!(events.first(), Some(Event::WorkspaceCreated(_))));
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::AppLaunched { .. })));
}
