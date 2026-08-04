mod workspaces;
use workspaces::{Cmd, Workspaces};

#[tauri::command]
fn ws_list(state: tauri::State<Workspaces>) -> String {
    state.call(Cmd::List)
}
#[tauri::command]
fn ws_create(name: String, chrome: bool, state: tauri::State<Workspaces>) -> String {
    state.call(Cmd::Create(name, chrome))
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
            ws_browser
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
