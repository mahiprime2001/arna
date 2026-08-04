//! The Docker runtime for WSE workspaces. A Docker workspace is a real sandbox:
//! its OWN filesystem, its OWN network (own container IP + mapped ports), isolated
//! from the host — exactly what the native runtime can't give. Inside it runs
//! code-server (VS Code in the browser), which the Arna UI embeds. We drive the
//! `docker` CLI (no SDK dependency).

use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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

// Cache availability so we don't shell out to `docker` on every UI poll (and so a
// machine without Docker stays snappy — WSE never requires Docker).
static AVAIL: Mutex<Option<(Instant, bool)>> = Mutex::new(None);

/// Is the Docker daemon reachable? (Docker Desktop running.) Cached ~8s.
pub fn available() -> bool {
    if let Ok(mut c) = AVAIL.lock() {
        if let Some((t, v)) = *c {
            if t.elapsed() < Duration::from_secs(8) {
                return v;
            }
        }
        let v = docker(&["version", "--format", "{{.Server.Version}}"]).is_some();
        *c = Some((Instant::now(), v));
        return v;
    }
    docker(&["version", "--format", "{{.Server.Version}}"]).is_some()
}

/// Create + start a code-server container for a workspace. Own filesystem
/// (a named volume), own network (container IP + a random host port → 8080).
///
/// The host port is published on **all interfaces** (`0.0.0.0`), not just
/// loopback, so a second machine on the same LAN can open the workspace's URL
/// (see [`lan_url`]). `--auth none` keeps the local embed frictionless — meaning
/// anyone who can reach the port while the container runs can open it, which is
/// fine on a trusted home/office LAN. Stop or destroy the workspace to close it.
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
        "0.0.0.0::8080", // random host port on ALL interfaces -> code-server 8080
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

/// The host port code-server is mapped to (Docker picks a random free one).
fn host_port(id: &str) -> Option<String> {
    let mapping = docker(&["port", &format!("wse-{id}"), "8080"])?;
    // e.g. "0.0.0.0:49153" or "[::]:49153" — the port is after the last ':'.
    let line = mapping.lines().next()?.trim();
    let port = line.rsplit(':').next()?.trim();
    if port.is_empty() {
        None
    } else {
        Some(port.to_string())
    }
}

/// The code-server URL for a running container, on **this** machine (loopback).
/// Used by the embedded Arna editor window.
pub fn url(id: &str) -> Option<String> {
    Some(format!("http://127.0.0.1:{}", host_port(id)?))
}

/// The URL a **second machine on the same LAN** opens to reach this workspace —
/// this host's LAN IP + the mapped port. `None` if the LAN IP can't be found or
/// the container isn't publishing yet.
pub fn lan_url(id: &str) -> Option<String> {
    Some(format!("http://{}:{}", lan_ip()?, host_port(id)?))
}

/// This machine's primary LAN IP. Opens a UDP socket toward a public address and
/// reads back the local address the OS would route through — no packets are sent,
/// it just resolves the outbound interface. Falls back to `None` if offline.
fn lan_ip() -> Option<String> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    Some(sock.local_addr().ok()?.ip().to_string())
}
