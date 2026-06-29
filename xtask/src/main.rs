//! Dev runner for Arna.
//!
//! `cargo dev` (a cargo alias → this crate) brings up the whole local stack with
//! one command: the signaling **backend**, then the unified **Arna app**
//! (`npm run tauri:dev`, which compiles, opens the window, and streams logs).
//!
//! On Windows each part runs in its own terminal window so you can watch the
//! logs and Ctrl+C either independently. Other platforms run them in the
//! background of the current shell.
//!
//! Tasks:
//!   cargo dev            # backend + app (default)
//!   cargo dev backend    # just the backend
//!   cargo dev app        # just the app

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The workspace root: `xtask/` sits directly under it.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has a parent dir")
        .to_path_buf()
}

fn main() {
    let task = std::env::args().nth(1).unwrap_or_else(|| "dev".into());
    match task.as_str() {
        "dev" => {
            start_backend();
            // Let the backend bind before the app's agent loop dials in.
            std::thread::sleep(Duration::from_secs(2));
            start_app();
            done();
        }
        "backend" => {
            start_backend();
            done();
        }
        "app" => {
            start_app();
            done();
        }
        other => {
            eprintln!("unknown task '{other}'.  try:  cargo dev  |  cargo dev backend  |  cargo dev app");
            std::process::exit(2);
        }
    }
}

fn start_backend() {
    println!("→ starting backend  (ws://127.0.0.1:8081/ws,  GET /health)");
    // Accounts (sign up / log in) require an SSO secret; set a dev one so the
    // login form works locally. ARNA_DEV_TICKETS enables the /dev/ticket helper.
    open_terminal(
        "Arna Backend",
        &root().join("backend"),
        "set ARNA_SSO_SECRET=arna-dev-secret && set ARNA_DEV_TICKETS=1 && cargo run",
    );
}

fn start_app() {
    // A stale Vite from a previous run holds the dev port; free it so the app
    // doesn't fail with "Port 4310 is already in use".
    free_port(4310);
    println!("→ starting Arna app (npm run tauri:dev — opens the window + logs)");
    // No ARNA_* env: the console UI defaults to the local backend, and with
    // accounts enabled this PC becomes reachable by pairing through the UI (sign
    // up → Add a device → tray "Pair this device"), not a token-less auto-start.
    open_terminal("Arna App", &root().join("console"), "npm run tauri:dev");
}

/// Kill whatever is listening on `port` (a leftover dev server from a prior run).
#[cfg(windows)]
fn free_port(port: u16) {
    let ps = format!(
        "Get-NetTCPConnection -LocalPort {port} -State Listen -ErrorAction SilentlyContinue | \
         ForEach-Object {{ Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue }}"
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .status();
}

#[cfg(not(windows))]
fn free_port(port: u16) {
    // Best-effort: kill the listener via fuser if available.
    let _ = Command::new("sh")
        .arg("-c")
        .arg(format!("fuser -k {port}/tcp 2>/dev/null || true"))
        .status();
}

fn done() {
    println!("\nLaunched. In the Arna window:");
    println!("  1. Sign up — any email + a password of 8+ characters (no preset account).");
    println!("  2. Add a device, then tray → \"Pair this device…\" and paste the id + token");
    println!("     to make THIS PC reachable. It then shows up under \"Your devices\".");
    println!("  (Backend runs with a dev SSO secret so accounts work locally.)");
    println!("\nClose those terminal windows (or Ctrl+C in them) to stop.");
}

/// Open a new terminal window that `cd`s into `dir` and runs `command`.
#[cfg(windows)]
fn open_terminal(title: &str, dir: &Path, command: &str) {
    use std::os::windows::process::CommandExt;
    // cmd /c start "<title>" cmd /k "cd /d <dir> && <command>"
    // raw_arg passes the line through verbatim so cmd (not Rust) does the quoting.
    let line = format!(
        r#"/c start "{title}" cmd /k "cd /d {} && {command}""#,
        dir.display()
    );
    if let Err(e) = Command::new("cmd").raw_arg(line).spawn() {
        eprintln!("failed to open terminal for {title}: {e}");
    }
}

/// Best-effort on non-Windows: run in the background of the current shell.
#[cfg(not(windows))]
fn open_terminal(title: &str, dir: &Path, command: &str) {
    eprintln!("[{title}]  ({}) $ {command}", dir.display());
    if let Err(e) = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .spawn()
    {
        eprintln!("failed to start {title}: {e}");
    }
}
