//! WSE Desktop shell (v0.1) — a thin, everyday driver over the engine + native
//! adapter. The point is to *live in it*, not to demo a framework. Four ideas:
//! create a workspace, enter it (switch to its desktop), launch apps, destroy it.
//!
//! It's an interactive shell (one process holds the running workspaces for your
//! session). Persisting workspaces across restarts is a daemon — a backlog item
//! for when daily use asks for it.

use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

use wse_adapter_windows_native::{
    enter_workspace_desktop, spawn_workspace_dock, switch_to_default_desktop, WindowsNativeAdapter,
};
use wse_common::{ApplicationDescriptor, Persistence, WorkspaceId, WorkspaceState};
use wse_engine::{Engine, WorkspaceConfig};

fn catalog() -> Vec<ApplicationDescriptor> {
    vec![
        ApplicationDescriptor::new("browser", "Browser"),
        ApplicationDescriptor::new("terminal", "Terminal"),
        ApplicationDescriptor::new("editor", "Editor"),
    ]
}

/// Map a friendly app name to a catalog entry (only isolatable apps exist today).
fn app_entry(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        "browser" | "chrome" | "edge" | "web" => Some("browser"),
        "terminal" | "term" | "cmd" | "shell" => Some("terminal"),
        "editor" | "code" | "notepad" => Some("editor"),
        _ => None,
    }
}

fn help() {
    println!(
        "\nWSE Desktop — commands:\n\
         \x20 create <name>            make a new workspace\n\
         \x20 ls                       list workspaces\n\
         \x20 launch <name> <app>      run an app in a workspace (browser|terminal|editor)\n\
         \x20 enter <name>             switch to the workspace's desktop  (Ctrl+Alt+Q to return)\n\
         \x20 back                     return to your normal desktop\n\
         \x20 suspend <name>           suspend a workspace\n\
         \x20 destroy <name>           destroy a workspace (removes its home)\n\
         \x20 help                     this\n\
         \x20 quit                     destroy all workspaces and exit\n"
    );
}

fn main() {
    let mut engine = Engine::new(WindowsNativeAdapter::new());
    let mut names: HashMap<String, WorkspaceId> = HashMap::new();
    let mut docked: HashSet<WorkspaceId> = HashSet::new();

    println!("WSE Desktop v0.1 — native Windows workspaces. Type 'help'.");
    let stdin = io::stdin();
    loop {
        print!("wse> ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap_or(0) == 0 {
            break; // EOF
        }
        // Tolerate a leading BOM (e.g. when a script is piped in as UTF-8-BOM).
        let parts: Vec<&str> = line.trim_start_matches('\u{feff}').split_whitespace().collect();
        let Some(&cmd) = parts.first() else { continue };

        match cmd {
            "help" | "?" => help(),
            "create" => match parts.get(1) {
                Some(&name) => {
                    let cfg = WorkspaceConfig::new(name, Persistence::Temporary, catalog());
                    match engine.create_workspace(cfg) {
                        Ok(id) => {
                            names.insert(name.to_string(), id);
                            println!("created workspace '{name}'");
                        }
                        Err(e) => println!("error: {e}"),
                    }
                }
                None => println!("usage: create <name>"),
            },
            "ls" | "list" => {
                if names.is_empty() {
                    println!("(no workspaces)");
                }
                for (name, id) in &names {
                    let state = engine.state(id).unwrap_or(WorkspaceState::Created);
                    let apps = engine.app_instances(id).map(|v| v.len()).unwrap_or(0);
                    println!("  {name:<16} {state:?}  ({apps} app(s))");
                }
            }
            "launch" => match (parts.get(1), parts.get(2)) {
                (Some(&name), Some(&app)) => match (names.get(name), app_entry(app)) {
                    (Some(id), Some(entry)) => {
                        let id = id.clone();
                        if engine.state(&id) != Some(WorkspaceState::Running) {
                            if let Err(e) = engine.start(&id) {
                                println!("error starting: {e}");
                                continue;
                            }
                        }
                        match engine.launch(&id, entry) {
                            Ok(_) => println!("launched {app} in '{name}'"),
                            Err(e) => println!("error: {e}"),
                        }
                    }
                    (None, _) => println!("no workspace '{name}'"),
                    (_, None) => println!("unknown app '{app}' (try: browser, terminal, editor)"),
                },
                _ => println!("usage: launch <name> <app>"),
            },
            "enter" | "open" => match parts.get(1).and_then(|n| names.get(*n)) {
                Some(id) => {
                    // Give the workspace a dock (its taskbar) the first time you
                    // enter it, so you can launch/restore/close apps from inside.
                    if docked.insert(id.clone()) {
                        spawn_workspace_dock(id);
                    }
                    enter_workspace_desktop(id);
                    println!("entered — dock is on the left; Ctrl+Alt+Q to return");
                }
                None => println!("usage: enter <name>"),
            },
            "back" => switch_to_default_desktop(),
            "suspend" => match parts.get(1).and_then(|n| names.get(*n)) {
                Some(id) => match engine.stop(&id.clone()) {
                    Ok(_) => println!("suspended"),
                    Err(e) => println!("error: {e}"),
                },
                None => println!("usage: suspend <name>"),
            },
            "destroy" => match parts.get(1) {
                Some(&name) => match names.remove(name) {
                    Some(id) => match engine.destroy(&id) {
                        Ok(_) => println!("destroyed '{name}'"),
                        Err(e) => println!("error: {e}"),
                    },
                    None => println!("no workspace '{name}'"),
                },
                None => println!("usage: destroy <name>"),
            },
            "quit" | "exit" => {
                for id in names.values() {
                    let _ = engine.destroy(id);
                }
                switch_to_default_desktop();
                println!("bye");
                break;
            }
            other => println!("unknown command '{other}' (try 'help')"),
        }
    }
}
