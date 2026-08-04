//! wse-server — the local WSE daemon. Holds the engine + workspaces and speaks a
//! tiny line protocol over 127.0.0.1 so a UI (Flutter) can drive them:
//!   - the client sends a text command per line: `create Work`, `launch <id>`, …
//!   - the server replies with one JSON line of state after every command.
//! It stays alive across UI connect/disconnect, so workspaces persist.

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use wse_adapter_windows_native::{
    enter_workspace_desktop, import_default_profile, set_preferred_browser, spawn_workspace_dock,
    WindowsNativeAdapter,
};
use wse_common::{ApplicationDescriptor, Persistence, WorkspaceId, WorkspaceState};
use wse_engine::{Engine, WorkspaceConfig};

const ADDR: &str = "127.0.0.1:47611";

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
            id, esc(&idy.name), state, apps
        ));
    }
    out.push_str("]}");
    out
}

fn handle_command(
    engine: &mut Engine<WindowsNativeAdapter>,
    docked: &mut HashSet<WorkspaceId>,
    line: &str,
) {
    let mut it = line.split_whitespace();
    let Some(cmd) = it.next() else { return };
    let arg = it.next().unwrap_or("");
    let arg2 = it.next().unwrap_or("");
    let wid = || WorkspaceId::from_raw(arg);

    match cmd {
        "list" => {}
        "create" => {
            let name = if arg.is_empty() { "Workspace" } else { arg };
            let cfg = WorkspaceConfig::new(name, Persistence::Temporary, catalog());
            let _ = engine.create_workspace(cfg);
        }
        "launch" => {
            let id = wid();
            if engine.state(&id) != Some(WorkspaceState::Running) {
                let _ = engine.start(&id);
            }
            let _ = engine.launch(&id, "browser");
        }
        "enter" => {
            let id = wid();
            if engine.state(&id) != Some(WorkspaceState::Running) {
                let _ = engine.start(&id);
            }
            if docked.insert(id.clone()) {
                spawn_workspace_dock(&id);
            }
            enter_workspace_desktop(&id);
        }
        "suspend" => {
            let _ = engine.stop(&wid());
        }
        "destroy" => {
            let id = wid();
            let _ = engine.destroy(&id);
            docked.remove(&id);
        }
        "import" => {
            let chrome = arg2.eq_ignore_ascii_case("chrome");
            let _ = import_default_profile(&wid(), chrome);
        }
        "browser" => {
            set_preferred_browser(arg.eq_ignore_ascii_case("chrome"));
        }
        _ => {}
    }
}

fn serve(engine: &mut Engine<WindowsNativeAdapter>, docked: &mut HashSet<WorkspaceId>, stream: TcpStream) {
    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(_) => return,
    };
    // initial state on connect
    let _ = writeln!(writer, "{}", state_json(engine));
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        handle_command(engine, docked, line);
        if writeln!(writer, "{}", state_json(engine)).is_err() {
            break;
        }
    }
}

fn main() {
    let listener = match TcpListener::bind(ADDR) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("wse-server: cannot bind {ADDR}: {e}");
            return;
        }
    };
    println!("wse-server listening on {ADDR}");

    // The engine lives for the whole daemon (not per connection) -> workspaces
    // persist across UI connect/disconnect.
    let mut engine = Engine::new(WindowsNativeAdapter::new());
    let mut docked: HashSet<WorkspaceId> = HashSet::new();

    for stream in listener.incoming() {
        match stream {
            Ok(s) => serve(&mut engine, &mut docked, s),
            Err(_) => continue,
        }
    }
}
