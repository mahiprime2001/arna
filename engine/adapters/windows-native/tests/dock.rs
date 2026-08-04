//! Live smoke test for the workspace dock. `#[ignore]` — it creates a real
//! desktop and a GUI window on it (invisible: it's on a separate desktop, not
//! your screen). Run: cargo test -p wse-adapter-windows-native --test dock -- --ignored --nocapture

use wse_adapter_windows_native::{desktop_window_titles, spawn_workspace_dock, WindowsNativeAdapter};
use wse_common::{Persistence, ResourceLimits, WorkspaceId};
use wse_contract::{WorkspaceAdapter, WorkspaceDef};

#[test]
#[ignore = "creates a real desktop + GUI window on it"]
fn dock_appears_on_the_workspace_desktop() {
    let mut adapter = WindowsNativeAdapter::new();
    let id = WorkspaceId::new();
    let def = WorkspaceDef {
        id: id.clone(),
        name: "dock-test".into(),
        persistence: Persistence::Temporary,
        limits: ResourceLimits {
            cpu_cores: None,
            memory_gb: None,
            storage_gb: None,
        },
    };
    adapter.create(&def).unwrap();

    spawn_workspace_dock(&id);
    std::thread::sleep(std::time::Duration::from_secs(2));

    let titles = desktop_window_titles(&id);
    println!("desktop windows: {titles:?}");
    assert!(
        titles.iter().any(|t| t == "WSE"),
        "the dock window ('WSE') should be present on the desktop, saw {titles:?}"
    );

    adapter.destroy(&id).unwrap();
    // After destroy the desktop is gone -> no windows.
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        desktop_window_titles(&id).is_empty(),
        "destroy should leave no windows / desktop"
    );
}
