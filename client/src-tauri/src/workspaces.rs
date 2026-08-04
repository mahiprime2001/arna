//! WSE workspaces embedded in the Tauri backend. The native engine holds raw
//! desktop handles (not Send/Sync), so it runs on its own dedicated thread and
//! Tauri commands talk to it over a channel. Every call returns the workspace
//! state as a JSON string the frontend parses.

use std::collections::HashSet;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::Mutex;
use std::thread;

use wse_adapter_windows_native::{
    enter_workspace_desktop, import_default_profile, set_preferred_browser, spawn_workspace_dock,
    WindowsNativeAdapter,
};
use wse_common::{ApplicationDescriptor, Persistence, WorkspaceId, WorkspaceState};
use wse_engine::{Engine, WorkspaceConfig};

pub enum Cmd {
    List,
    Create(String, bool), // name, prefer chrome
    Start(String),
    Launch(String),
    Enter(String),
    Suspend(String),
    Destroy(String),
    Import(String, bool), // id, chrome
    Browser(bool),        // chrome
}

struct Req {
    cmd: Cmd,
    reply: SyncSender<String>,
}

/// Handle the frontend talks to. Send + Sync (the non-Send engine stays on its
/// own thread behind the channel).
pub struct Workspaces {
    tx: Mutex<SyncSender<Req>>,
}

impl Workspaces {
    pub fn new() -> Self {
        let (tx, rx) = sync_channel::<Req>(64);
        thread::spawn(move || {
            let mut engine = Engine::new(WindowsNativeAdapter::new());
            let mut docked: HashSet<WorkspaceId> = HashSet::new();
            let mut chrome: std::collections::HashMap<WorkspaceId, bool> =
                std::collections::HashMap::new();
            while let Ok(req) = rx.recv() {
                exec(&mut engine, &mut docked, &mut chrome, req.cmd);
                let _ = req.reply.send(state_json(&mut engine));
            }
        });
        Workspaces { tx: Mutex::new(tx) }
    }

    pub fn call(&self, cmd: Cmd) -> String {
        let (rtx, rrx) = sync_channel(1);
        if let Ok(tx) = self.tx.lock() {
            if tx.send(Req { cmd, reply: rtx }).is_ok() {
                return rrx.recv().unwrap_or_else(|_| empty());
            }
        }
        empty()
    }
}

fn empty() -> String {
    "{\"workspaces\":[]}".into()
}

fn catalog() -> Vec<ApplicationDescriptor> {
    vec![
        ApplicationDescriptor::new("browser", "Browser"),
        ApplicationDescriptor::new("terminal", "Terminal"),
        ApplicationDescriptor::new("editor", "Editor"),
    ]
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn state_json(engine: &mut Engine<WindowsNativeAdapter>) -> String {
    let mut out = String::from("{\"workspaces\":[");
    for (i, id) in engine.workspace_ids().into_iter().enumerate() {
        let Some(idy) = engine.identity(&id) else { continue };
        let state = match idy.state {
            WorkspaceState::Running => "running",
            WorkspaceState::Saved => "suspended",
            WorkspaceState::Created => "ready",
            _ => "gone",
        };
        let apps = if idy.state == WorkspaceState::Running {
            engine.app_instances(&id).map(|v| v.len()).unwrap_or(0)
        } else {
            0
        };
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"id\":\"{}\",\"name\":\"{}\",\"state\":\"{}\",\"apps\":{}}}",
            id,
            esc(&idy.name),
            state,
            apps
        ));
    }
    out.push_str("]}");
    out
}

fn exec(
    engine: &mut Engine<WindowsNativeAdapter>,
    docked: &mut HashSet<WorkspaceId>,
    chrome: &mut std::collections::HashMap<WorkspaceId, bool>,
    cmd: Cmd,
) {
    match cmd {
        Cmd::List => {}
        Cmd::Create(name, prefer_chrome) => {
            let name = if name.trim().is_empty() { "Workspace".to_string() } else { name };
            let cfg = WorkspaceConfig::new(&name, Persistence::Temporary, catalog());
            if let Ok(id) = engine.create_workspace(cfg) {
                chrome.insert(id, prefer_chrome);
            }
        }
        Cmd::Start(id) => {
            let _ = engine.start(&WorkspaceId::from_raw(id));
        }
        Cmd::Launch(id) => {
            let id = WorkspaceId::from_raw(id);
            set_preferred_browser(chrome.get(&id).copied().unwrap_or(false));
            if engine.state(&id) != Some(WorkspaceState::Running) {
                let _ = engine.start(&id);
            }
            let _ = engine.launch(&id, "browser");
        }
        Cmd::Enter(id) => {
            let id = WorkspaceId::from_raw(id);
            set_preferred_browser(chrome.get(&id).copied().unwrap_or(false));
            if engine.state(&id) != Some(WorkspaceState::Running) {
                let _ = engine.start(&id);
            }
            // Don't drop you onto an empty desktop: launch the workspace's browser
            // if nothing is running yet.
            let empty = engine.app_instances(&id).map(|v| v.is_empty()).unwrap_or(true);
            if empty {
                let _ = engine.launch(&id, "browser");
            }
            if docked.insert(id.clone()) {
                spawn_workspace_dock(&id);
            }
            enter_workspace_desktop(&id);
        }
        Cmd::Suspend(id) => {
            let _ = engine.stop(&WorkspaceId::from_raw(id));
        }
        Cmd::Destroy(id) => {
            let id = WorkspaceId::from_raw(id);
            let _ = engine.destroy(&id);
            docked.remove(&id);
            chrome.remove(&id);
        }
        Cmd::Import(id, is_chrome) => {
            let _ = import_default_profile(&WorkspaceId::from_raw(id), is_chrome);
        }
        Cmd::Browser(is_chrome) => set_preferred_browser(is_chrome),
    }
}
