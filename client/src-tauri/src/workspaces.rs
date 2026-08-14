//! WSE workspaces embedded in the Tauri backend. Two runtimes behind one UI:
//!  - **native** — separate Windows desktop + real Windows apps (no strong
//!    isolation). The native engine holds raw desktop handles (not Send/Sync),
//!    so everything runs on this one dedicated thread.
//!  - **docker** — a real sandbox container: own filesystem, own network (own IP
//!    + mapped ports), running code-server the Arna UI embeds.
//! The engine is unchanged; docker is simply another runtime alongside it. Every
//! call returns the merged workspace state as JSON.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
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

// ── Workspace persistence (close WSE -> reopen -> resume) ─────────────────────
// One registry records every workspace's identity + how to reconstruct it.
//  - Docker: state is on disk (container + volume); we persist id->name and, on
//    reopen, docker start resumes it.
//  - Native: state (home/profiles/overlay) is on disk keyed by id; we persist the
//    metadata and, on Resume, `engine.restore(id, ..)` re-adopts the SAME id
//    (ADR-0011) so files match, then start + launch reconstruct the runtime.
#[derive(serde::Serialize, serde::Deserialize)]
struct RegEntry {
    id: String,
    name: String,
    runtime: String, // "native" | "docker"
    #[serde(default)]
    apps: Vec<String>,
    #[serde(default)]
    chrome: bool,
}

/// A persisted native workspace not yet resumed this session (suspended).
struct SavedWs {
    name: String,
    apps: Vec<String>,
    chrome: bool,
}

fn wse_dir() -> PathBuf {
    std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join(".wse")
}

fn registry_path() -> PathBuf {
    wse_dir().join("registry.json")
}

fn native_home(id: &str) -> PathBuf {
    wse_dir().join("workspaces").join(format!("wse-{id}"))
}

/// Load persisted workspaces, pruning ghosts (docker container removed, or native
/// home deleted). Returns (docker id->name, saved native id->SavedWs).
fn load_registry() -> (HashMap<String, String>, HashMap<String, SavedWs>) {
    let mut dockers = HashMap::new();
    let mut saved = HashMap::new();
    if let Ok(s) = std::fs::read_to_string(registry_path()) {
        if let Ok(entries) = serde_json::from_str::<Vec<RegEntry>>(&s) {
            for e in entries {
                if e.runtime == "docker" {
                    if docker::exists(&e.id) {
                        dockers.insert(e.id, e.name);
                    }
                } else if native_home(&e.id).exists() {
                    saved.insert(e.id, SavedWs { name: e.name, apps: e.apps, chrome: e.chrome });
                }
            }
        }
    }
    (dockers, saved)
}

/// Write the current workspace set to the registry (full rewrite; a destroyed
/// workspace simply isn't included).
fn persist(rt: &Rt) {
    let mut entries: Vec<RegEntry> = Vec::new();
    // Live native workspaces (engine-tracked).
    for id in rt.engine.workspace_ids() {
        if let Some(idy) = rt.engine.identity(&id) {
            entries.push(RegEntry {
                id: id.to_string(),
                name: idy.name,
                runtime: "native".into(),
                apps: rt.apps.get(&id).cloned().unwrap_or_default(),
                chrome: rt.chrome.get(&id).copied().unwrap_or(false),
            });
        }
    }
    // Suspended natives not yet resumed this session.
    for (id, sv) in &rt.saved {
        entries.push(RegEntry {
            id: id.clone(),
            name: sv.name.clone(),
            runtime: "native".into(),
            apps: sv.apps.clone(),
            chrome: sv.chrome,
        });
    }
    // Docker workspaces.
    for (id, name) in &rt.dockers {
        entries.push(RegEntry {
            id: id.clone(),
            name: name.clone(),
            runtime: "docker".into(),
            apps: Vec::new(),
            chrome: false,
        });
    }
    if let Ok(s) = serde_json::to_string(&entries) {
        let p = registry_path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, s);
    }
}

/// Bring a persisted native workspace back into the engine under its EXISTING id
/// (ADR-0011), so its on-disk home/profiles/overlay match. No-op if not saved.
fn ensure_restored(rt: &mut Rt, id: &str) {
    if let Some(sv) = rt.saved.remove(id) {
        let wid = WorkspaceId::from_raw(id.to_string());
        let cfg = WorkspaceConfig::new(&sv.name, Persistence::Temporary, catalog());
        if rt.engine.restore(wid.clone(), cfg).is_ok() {
            rt.chrome.insert(wid.clone(), sv.chrome);
            rt.apps.insert(wid, sv.apps);
        } else {
            rt.saved.insert(id.to_string(), sv); // restore failed; don't lose it
        }
    }
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
    /// Persisted native workspaces not yet resumed this session (suspended).
    saved: HashMap<String, SavedWs>,
}

impl Workspaces {
    pub fn new() -> Self {
        let (tx, rx) = sync_channel::<Req>(64);
        thread::spawn(move || {
            // Persisted workspaces survive a restart (ADR-0011): Docker via its
            // container/volume, native via engine.restore on resume.
            let (dockers, saved) = load_registry();
            let mut rt = Rt {
                engine: Engine::new(WindowsNativeAdapter::new()),
                docked: HashSet::new(),
                chrome: HashMap::new(),
                apps: HashMap::new(),
                dockers,
                saved,
            };
            while let Ok(req) = rx.recv() {
                let persist_after = !matches!(req.cmd, Cmd::List);
                exec(&mut rt, req.cmd);
                if persist_after {
                    persist(&rt);
                }
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

/// A runtime's honest guarantees as JSON — the UI renders this straight, without
/// knowing which runtime produced it. Single source of truth: the WRM descriptors.
fn guarantees_json(g: &wse_wrm::Guarantees) -> String {
    format!(
        "{{\"environment\":\"{}\",\"workingDirectory\":\"{}\",\"overlay\":\"{}\",\"processTree\":\"{}\",\"clipboard\":\"{}\",\"registry\":\"{}\",\"network\":\"{}\"}}",
        g.environment.label(),
        g.working_directory.label(),
        g.overlay.label(),
        g.process_tree.label(),
        g.clipboard.label(),
        g.registry.label(),
        g.network.label(),
    )
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
            "{{\"id\":\"{}\",\"name\":\"{}\",\"runtime\":\"native\",\"state\":\"{}\",\"apps\":{},\"url\":null,\"guarantees\":{}}}",
            id, esc(&idy.name), state, apps,
            guarantees_json(&wse_wrm::runtimes::NATIVE_WINDOWS.guarantees)
        ));
    }

    // Suspended native workspaces: persisted, not yet resumed this session.
    for (id, sv) in &rt.saved {
        items.push(format!(
            "{{\"id\":\"{}\",\"name\":\"{}\",\"runtime\":\"native\",\"state\":\"suspended\",\"apps\":0,\"url\":null,\"guarantees\":{}}}",
            id,
            esc(&sv.name),
            guarantees_json(&wse_wrm::runtimes::NATIVE_WINDOWS.guarantees)
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
            "{{\"id\":\"{}\",\"name\":\"{}\",\"runtime\":\"docker\",\"state\":\"{}\",\"apps\":{},\"url\":{},\"lanUrl\":{},\"guarantees\":{}}}",
            id, esc(name), state, if running { 1 } else { 0 }, url, lan,
            guarantees_json(&wse_wrm::runtimes::DOCKER.guarantees)
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
        // Destroy a suspended (persisted, not-yet-resumed) native workspace.
        Cmd::Destroy(id) if rt.saved.contains_key(&id) => {
            rt.saved.remove(&id);
            let _ = std::fs::remove_dir_all(native_home(&id));
        }
        // ── native paths ─────────────────────────────────────────────────────
        Cmd::Start(id) => {
            ensure_restored(rt, &id); // resume a persisted workspace if needed
            let _ = rt.engine.start(&WorkspaceId::from_raw(id));
        }
        Cmd::Launch(id) => {
            ensure_restored(rt, &id);
            let id = WorkspaceId::from_raw(id);
            set_preferred_browser(rt.chrome.get(&id).copied().unwrap_or(false));
            if rt.engine.state(&id) != Some(WorkspaceState::Running) {
                let _ = rt.engine.start(&id);
            }
            let _ = rt.engine.launch(&id, "browser");
        }
        Cmd::Enter(id) => {
            ensure_restored(rt, &id); // resume a persisted workspace if needed
            let id = WorkspaceId::from_raw(id);
            set_preferred_browser(rt.chrome.get(&id).copied().unwrap_or(false));
            if rt.engine.state(&id) != Some(WorkspaceState::Running) {
                let _ = rt.engine.start(&id);
            }
            let entries = rt.apps.get(&id).cloned().unwrap_or_else(|| vec!["browser".into()]);
            let empty = rt.engine.app_instances(&id).map(|v| v.is_empty()).unwrap_or(true);
            if empty {
                for entry in &entries {
                    let _ = rt.engine.launch(&id, entry);
                }
            }
            if rt.docked.insert(id.clone()) {
                spawn_workspace_dock(&id, &entries);
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
