mod docker;
mod remote;
mod workspaces;
use std::path::Path;
use workspaces::{Cmd, Workspaces};

// ── Watch & Control: host-side remote session enforcement ────────────────────
#[tauri::command]
fn remote_join(workspace: String, guest: String, name: String) -> String {
    remote::join(&workspace, &guest, &name);
    remote::state_json(&workspace)
}
#[tauri::command]
fn remote_grant(workspace: String, guest: String) -> String {
    remote::grant(&workspace, &guest);
    remote::state_json(&workspace)
}
#[tauri::command]
fn remote_revoke(workspace: String) -> String {
    remote::revoke(&workspace);
    remote::state_json(&workspace)
}
#[tauri::command]
fn remote_disconnect(workspace: String, guest: String) -> String {
    remote::disconnect(&workspace, &guest);
    remote::state_json(&workspace)
}
/// Gated: injects into the OS only if the guest is the Controller. Returns
/// whether it was injected (false = rejected at the enforcement point).
#[tauri::command]
fn remote_input(workspace: String, guest: String, event: String) -> bool {
    remote::input(&workspace, &guest, &event)
}
#[tauri::command]
fn remote_session(workspace: String) -> String {
    remote::state_json(&workspace)
}

/// One captured frame of a NATIVE workspace's surface as a JPEG data URL (empty
/// string if nothing to capture yet). The frontend polls this into a <canvas> and
/// streams it over WebRTC — the native counterpart to Docker's code-server.
#[tauri::command]
fn remote_capture_frame(workspace: String) -> String {
    use base64::Engine;
    let wid = WorkspaceId::from_raw(workspace);
    let Some((w, h, rgb)) = wse_adapter_windows_native::capture_workspace_frame(&wid) else {
        return String::new();
    };
    let Some(img) = image::RgbImage::from_raw(w, h, rgb) else {
        return String::new();
    };
    let mut buf = Vec::new();
    if image::DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg)
        .is_err()
    {
        return String::new();
    }
    format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&buf)
    )
}

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
            ws_overlay_discard, remote_join, remote_grant, remote_revoke, remote_disconnect,
            remote_input, remote_session, remote_capture_frame
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
