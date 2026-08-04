//! The Docker runtime for WSE workspaces. A Docker workspace is a real sandbox:
//! its OWN filesystem, its OWN network (own container IP + mapped ports), isolated
//! from the host — exactly what the native runtime can't give. Inside it runs
//! code-server (VS Code in the browser), which the Arna UI embeds. We drive the
//! `docker` CLI (no SDK dependency).

use std::process::Command;

/// The base image: Ubuntu + code-server (full VS Code, runs your code inside).
const IMAGE: &str = "codercom/code-server:latest";

fn docker(args: &[&str]) -> Option<String> {
    let out = Command::new("docker").args(args).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

/// Is the Docker daemon reachable? (Docker Desktop running.)
pub fn available() -> bool {
    docker(&["version", "--format", "{{.Server.Version}}"]).is_some()
}

/// Create + start a code-server container for a workspace. Own filesystem
/// (a named volume), own network (container IP + a random localhost port → 8080).
pub fn create(id: &str) -> bool {
    let name = format!("wse-{id}");
    let vol = format!("wse-{id}-data:/home/coder");
    docker(&[
        "run",
        "-d",
        "--name",
        &name,
        "--hostname",
        "workspace",
        "-p",
        "127.0.0.1::8080", // random host port -> code-server 8080
        "-v",
        &vol,
        IMAGE,
        "--auth",
        "none", // local embed; no password prompt
        "--bind-addr",
        "0.0.0.0:8080",
    ])
    .is_some()
}

pub fn start(id: &str) -> bool {
    docker(&["start", &format!("wse-{id}")]).is_some()
}

pub fn stop(id: &str) -> bool {
    docker(&["stop", &format!("wse-{id}")]).is_some()
}

pub fn destroy(id: &str) {
    let name = format!("wse-{id}");
    let vol = format!("wse-{id}-data");
    let _ = docker(&["rm", "-f", &name]);
    let _ = docker(&["volume", "rm", &vol]);
}

pub fn running(id: &str) -> bool {
    docker(&["inspect", "-f", "{{.State.Running}}", &format!("wse-{id}")])
        .map(|s| s == "true")
        .unwrap_or(false)
}

/// The code-server URL for a running container (from the mapped host port).
pub fn url(id: &str) -> Option<String> {
    let mapping = docker(&["port", &format!("wse-{id}"), "8080"])?;
    let hostport = mapping.lines().next()?.trim();
    if hostport.is_empty() {
        return None;
    }
    // docker may print 0.0.0.0:PORT — normalise to localhost.
    let hostport = hostport.replace("0.0.0.0", "127.0.0.1");
    Some(format!("http://{hostport}"))
}
