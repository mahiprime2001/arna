mod docker;
mod workspaces;
use std::path::Path;
use workspaces::{Cmd, Workspaces};

use wse_adapter_windows_native::{
    overlay_changes, overlay_discard, overlay_import, overlay_list, overlay_merge,
};
use wse_common::WorkspaceId;

// ── Overlay Review: a workspace owns its file CHANGES, not your originals ─────
// Stateless filesystem ops keyed by workspace id, so they run straight on the
// command thread (no engine round-trip). Share a host folder -> the workspace
// works on a copy -> review the diff -> merge back or discard.

#[tauri::command]
fn ws_overlay_share(id: String, host: String) -> String {
    let wid = WorkspaceId::from_raw(id);
    match overlay_import(&wid, Path::new(host.trim())) {
        Some(name) => serde_json::json!({ "ok": true, "name": name }).to_string(),
        None => serde_json::json!({ "ok": false }).to_string(),
    }
}

#[tauri::command]
fn ws_overlay_list(id: String) -> String {
    let wid = WorkspaceId::from_raw(id);
    serde_json::json!({ "overlays": overlay_list(&wid) }).to_string()
}

#[tauri::command]
fn ws_overlay_changes(id: String, name: String) -> String {
    let wid = WorkspaceId::from_raw(id);
    let changes: Vec<_> = overlay_changes(&wid, &name)
        .into_iter()
        .map(|c| serde_json::json!({ "rel": c.rel, "kind": c.kind }))
        .collect();
    serde_json::json!({ "name": name, "changes": changes }).to_string()
}

#[tauri::command]
fn ws_overlay_merge(id: String, name: String) -> String {
    let wid = WorkspaceId::from_raw(id);
    serde_json::json!({ "merged": overlay_merge(&wid, &name) }).to_string()
}

#[tauri::command]
fn ws_overlay_discard(id: String, name: String) -> String {
    let wid = WorkspaceId::from_raw(id);
    overlay_discard(&wid, &name);
    serde_json::json!({ "ok": true }).to_string()
}

#[tauri::command]
fn ws_list(state: tauri::State<Workspaces>) -> String {
    state.call(Cmd::List)
}
#[tauri::command]
fn ws_create(
    name: String,
    runtime: String,
    apps: Vec<String>,
    state: tauri::State<Workspaces>,
) -> String {
    state.call(Cmd::Create(name, runtime, apps))
}
#[tauri::command]
fn ws_start(id: String, state: tauri::State<Workspaces>) -> String {
    state.call(Cmd::Start(id))
}
#[tauri::command]
fn ws_launch(id: String, state: tauri::State<Workspaces>) -> String {
    state.call(Cmd::Launch(id))
}
#[tauri::command]
fn ws_enter(id: String, state: tauri::State<Workspaces>) -> String {
    state.call(Cmd::Enter(id))
}
#[tauri::command]
fn ws_suspend(id: String, state: tauri::State<Workspaces>) -> String {
    state.call(Cmd::Suspend(id))
}
#[tauri::command]
fn ws_destroy(id: String, state: tauri::State<Workspaces>) -> String {
    state.call(Cmd::Destroy(id))
}
#[tauri::command]
fn ws_import(id: String, chrome: bool, state: tauri::State<Workspaces>) -> String {
    state.call(Cmd::Import(id, chrome))
}
#[tauri::command]
fn ws_browser(chrome: bool, state: tauri::State<Workspaces>) -> String {
    state.call(Cmd::Browser(chrome))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Workspaces::new())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ws_list, ws_create, ws_start, ws_launch, ws_enter, ws_suspend, ws_destroy, ws_import,
            ws_browser, ws_overlay_share, ws_overlay_list, ws_overlay_changes, ws_overlay_merge,
            ws_overlay_discard
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
