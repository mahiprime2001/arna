//! WSE workspaces embedded in the Tauri backend. Two runtimes behind one UI:
//!  - **native** — separate Windows desktop + real Windows apps (no strong
//!    isolation). The native engine holds raw desktop handles (not Send/Sync),
//!    so everything runs on this one dedicated thread.
//!  - **docker** — a real sandbox container: own filesystem, own network (own IP
//!    + mapped ports), running code-server the Arna UI embeds.
//! The engine is unchanged; docker is simply another runtime alongside it. Every
//! call returns the merged workspace state as JSON.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::Mutex;
use std::thread;

use wse_adapter_windows_native::{
    enter_workspace_desktop, import_default_profile, set_preferred_browser, spawn_workspace_dock,
    WindowsNativeAdapter,
};
use wse_common::{ApplicationDescriptor, Persistence, WorkspaceId, WorkspaceState};
use wse_engine::{Engine, WorkspaceConfig};

use crate::docker;

pub enum Cmd {
    List,
    Create(String, String, Vec<String>), // name, runtime, selected UI app ids
    Start(String),
    Launch(String),
    Enter(String),
    Suspend(String),
    Destroy(String),
    Import(String, bool), // id, chrome
    Browser(bool),        // chrome
}

fn map_app(ui: &str) -> Option<&'static str> {
    match ui {
        "chrome" | "edge" | "browser" => Some("browser"),
        "vscode" | "code" | "editor" => Some("editor"),
        "terminal" | "term" => Some("terminal"),
        _ => None,
    }
}

struct Req {
    cmd: Cmd,
    reply: SyncSender<String>,
}

pub struct Workspaces {
    tx: Mutex<SyncSender<Req>>,
}

/// Everything the runtime thread owns.
struct Rt {
    engine: Engine<WindowsNativeAdapter>,
    docked: HashSet<WorkspaceId>,
    chrome: HashMap<WorkspaceId, bool>,
    apps: HashMap<WorkspaceId, Vec<String>>,
    /// Docker workspaces: id -> display name.
    dockers: HashMap<String, String>,
}

impl Workspaces {
    pub fn new() -> Self {
        let (tx, rx) = sync_channel::<Req>(64);
        thread::spawn(move || {
            let mut rt = Rt {
                engine: Engine::new(WindowsNativeAdapter::new()),
                docked: HashSet::new(),
                chrome: HashMap::new(),
                apps: HashMap::new(),
                dockers: HashMap::new(),
            };
            while let Ok(req) = rx.recv() {
                exec(&mut rt, req.cmd);
                let _ = req.reply.send(state_json(&mut rt));
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
    "{\"workspaces\":[],\"docker\":false}".into()
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

fn state_json(rt: &mut Rt) -> String {
    let mut items: Vec<String> = Vec::new();

    // Native workspaces.
    for id in rt.engine.workspace_ids() {
        let Some(idy) = rt.engine.identity(&id) else { continue };
        let state = match idy.state {
            WorkspaceState::Running => "running",
            WorkspaceState::Saved => "suspended",
            WorkspaceState::Created => "ready",
            _ => "gone",
        };
        let apps = if idy.state == WorkspaceState::Running {
            rt.engine.app_instances(&id).map(|v| v.len()).unwrap_or(0)
        } else {
            0
        };
        items.push(format!(
            "{{\"id\":\"{}\",\"name\":\"{}\",\"runtime\":\"native\",\"state\":\"{}\",\"apps\":{},\"url\":null}}",
            id, esc(&idy.name), state, apps
        ));
    }

    // Docker workspaces.
    for (id, name) in &rt.dockers {
        let running = docker::running(id);
        let state = if running { "running" } else { "suspended" };
        let url = match docker::url(id) {
            Some(u) if running => format!("\"{}\"", esc(&u)),
            _ => "null".into(),
        };
        let lan = match docker::lan_url(id) {
            Some(u) if running => format!("\"{}\"", esc(&u)),
            _ => "null".into(),
        };
        items.push(format!(
            "{{\"id\":\"{}\",\"name\":\"{}\",\"runtime\":\"docker\",\"state\":\"{}\",\"apps\":{},\"url\":{},\"lanUrl\":{}}}",
            id, esc(name), state, if running { 1 } else { 0 }, url, lan
        ));
    }

    format!(
        "{{\"workspaces\":[{}],\"docker\":{}}}",
        items.join(","),
        docker::available()
    )
}

fn exec(rt: &mut Rt, cmd: Cmd) {
    match cmd {
        Cmd::List => {}
        Cmd::Create(name, runtime, ui_apps) => {
            let name = if name.trim().is_empty() { "Workspace".to_string() } else { name };
            if runtime == "docker" {
                let id = WorkspaceId::new().to_string();
                if docker::create(&id) {
                    rt.dockers.insert(id, name);
                }
            } else {
                let cfg = WorkspaceConfig::new(&name, Persistence::Temporary, catalog());
                if let Ok(id) = rt.engine.create_workspace(cfg) {
                    rt.chrome.insert(id.clone(), ui_apps.iter().any(|a| a == "chrome"));
                    let mut entries: Vec<String> =
                        ui_apps.iter().filter_map(|a| map_app(a)).map(String::from).collect();
                    entries.dedup();
                    if entries.is_empty() {
                        entries.push("browser".into());
                    }
                    rt.apps.insert(id, entries);
                }
            }
        }
        Cmd::Start(id) | Cmd::Launch(id) if rt.dockers.contains_key(&id) => {
            docker::start(&id);
        }
        Cmd::Enter(id) if rt.dockers.contains_key(&id) => {
            // Docker "enter" = make sure it's running; the UI opens code-server.
            if !docker::running(&id) {
                docker::start(&id);
            }
        }
        Cmd::Suspend(id) if rt.dockers.contains_key(&id) => {
            docker::stop(&id);
        }
        Cmd::Destroy(id) if rt.dockers.contains_key(&id) => {
            docker::destroy(&id);
            rt.dockers.remove(&id);
        }
        // ── native paths ─────────────────────────────────────────────────────
        Cmd::Start(id) => {
            let _ = rt.engine.start(&WorkspaceId::from_raw(id));
        }
        Cmd::Launch(id) => {
            let id = WorkspaceId::from_raw(id);
            set_preferred_browser(rt.chrome.get(&id).copied().unwrap_or(false));
            if rt.engine.state(&id) != Some(WorkspaceState::Running) {
                let _ = rt.engine.start(&id);
            }
            let _ = rt.engine.launch(&id, "browser");
        }
        Cmd::Enter(id) => {
            let id = WorkspaceId::from_raw(id);
            set_preferred_browser(rt.chrome.get(&id).copied().unwrap_or(false));
            if rt.engine.state(&id) != Some(WorkspaceState::Running) {
                let _ = rt.engine.start(&id);
            }
            let empty = rt.engine.app_instances(&id).map(|v| v.is_empty()).unwrap_or(true);
            if empty {
                let want = rt.apps.get(&id).cloned().unwrap_or_else(|| vec!["browser".into()]);
                for entry in want {
                    let _ = rt.engine.launch(&id, &entry);
                }
            }
            if rt.docked.insert(id.clone()) {
                spawn_workspace_dock(&id);
            }
            enter_workspace_desktop(&id);
        }
        Cmd::Suspend(id) => {
            let _ = rt.engine.stop(&WorkspaceId::from_raw(id));
        }
        Cmd::Destroy(id) => {
            let id = WorkspaceId::from_raw(id);
            let _ = rt.engine.destroy(&id);
            rt.docked.remove(&id);
            rt.chrome.remove(&id);
            rt.apps.remove(&id);
        }
        Cmd::Import(id, is_chrome) => {
            let _ = import_default_profile(&WorkspaceId::from_raw(id), is_chrome);
        }
        Cmd::Browser(is_chrome) => set_preferred_browser(is_chrome),
    }
}
