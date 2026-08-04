//! Workspace Overlay — a workspace owns its file CHANGES, not your originals.
//!
//! You share a host folder into a workspace; WSE copies it into the workspace's
//! overlay and the workspace's apps work on that copy. Nothing touches your real
//! folder until you **Review** the changes and **Merge** (or **Discard**). It's a
//! git working copy for any folder — the way the native runtime virtualises files
//! without fighting Windows.
//!
//! v1 is a copied overlay + diff/merge. A *live* copy-on-write layer (Windows
//! ProjFS) is a later upgrade behind this same API. Regenerable/huge trees
//! (.git, node_modules, build outputs) are skipped — you don't overlay those.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::workspace_home;
use wse_common::WorkspaceId;

const SKIP: &[&str] = &[
    ".git", "node_modules", "target", ".next", "dist", "build", "__pycache__", ".venv",
];

/// A change in the overlay relative to the original host folder.
pub struct Change {
    pub rel: String,
    pub kind: &'static str, // "added" | "modified" | "deleted"
}

fn overlay_root(id: &WorkspaceId) -> PathBuf {
    workspace_home(id).join("overlay")
}

fn origin_file(id: &WorkspaceId, name: &str) -> PathBuf {
    overlay_root(id).join(format!("{name}.origin"))
}

/// The overlay copy's path for a shared folder.
pub fn overlay_path(id: &WorkspaceId, name: &str) -> PathBuf {
    overlay_root(id).join(name)
}

/// Import a host folder as an overlay copy. Returns its name (the basename).
pub fn import(id: &WorkspaceId, host: &Path) -> Option<String> {
    let name = host.file_name()?.to_string_lossy().into_owned();
    let dst = overlay_path(id, &name);
    let _ = fs::remove_dir_all(&dst);
    let _ = fs::create_dir_all(overlay_root(id));
    copy_dir(host, &dst);
    let _ = fs::write(origin_file(id, &name), host.to_string_lossy().as_bytes());
    Some(name)
}

fn origin_of(id: &WorkspaceId, name: &str) -> Option<PathBuf> {
    let s = fs::read_to_string(origin_file(id, name)).ok()?;
    Some(PathBuf::from(s.trim()))
}

/// The overlays a workspace currently holds.
pub fn list(id: &WorkspaceId) -> Vec<String> {
    fs::read_dir(overlay_root(id))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .collect()
}

/// What changed in the overlay vs the original host folder.
pub fn changes(id: &WorkspaceId, name: &str) -> Vec<Change> {
    match origin_of(id, name) {
        Some(origin) => diff(&overlay_path(id, name), &origin),
        None => Vec::new(),
    }
}

/// Apply the overlay's changes back to the host folder. Returns the count applied.
pub fn merge(id: &WorkspaceId, name: &str) -> usize {
    match origin_of(id, name) {
        Some(origin) => apply(&overlay_path(id, name), &origin),
        None => 0,
    }
}

/// Diff an overlay tree against an origin tree (path-level; unit-testable).
fn diff(over: &Path, origin: &Path) -> Vec<Change> {
    let o = walk(over);
    let h = walk(origin);
    let mut out = Vec::new();
    for (rel, (sz, mt)) in &o {
        match h.get(rel) {
            None => out.push(Change { rel: rel.clone(), kind: "added" }),
            Some((hsz, hmt)) if sz != hsz || mt != hmt => {
                out.push(Change { rel: rel.clone(), kind: "modified" })
            }
            _ => {}
        }
    }
    for rel in h.keys() {
        if !o.contains_key(rel) {
            out.push(Change { rel: rel.clone(), kind: "deleted" });
        }
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

/// Apply an overlay's changes onto an origin tree (path-level; unit-testable).
fn apply(over: &Path, origin: &Path) -> usize {
    let mut n = 0;
    for c in diff(over, origin) {
        let dst = origin.join(&c.rel);
        match c.kind {
            "added" | "modified" => {
                if let Some(p) = dst.parent() {
                    let _ = fs::create_dir_all(p);
                }
                if fs::copy(over.join(&c.rel), &dst).is_ok() {
                    n += 1;
                }
            }
            "deleted" => {
                if fs::remove_file(&dst).is_ok() {
                    n += 1;
                }
            }
            _ => {}
        }
    }
    n
}

/// Throw the overlay away; the host folder is untouched.
pub fn discard(id: &WorkspaceId, name: &str) {
    let _ = fs::remove_dir_all(overlay_path(id, name));
    let _ = fs::remove_file(origin_file(id, name));
}

fn skipped(name: &str) -> bool {
    SKIP.iter().any(|s| s.eq_ignore_ascii_case(name))
}

/// Recursive copy that preserves mtimes (so the diff is fast + accurate) and
/// skips regenerable/huge trees.
fn copy_dir(src: &Path, dst: &Path) {
    let _ = fs::create_dir_all(dst);
    let Ok(entries) = fs::read_dir(src) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name();
        if skipped(&name.to_string_lossy()) {
            continue;
        }
        let (sp, dp) = (e.path(), dst.join(&name));
        match e.file_type() {
            Ok(t) if t.is_dir() => copy_dir(&sp, &dp),
            _ => {
                if fs::copy(&sp, &dp).is_ok() {
                    if let Ok(m) = fs::metadata(&sp).and_then(|md| md.modified()) {
                        if let Ok(f) = fs::File::options().write(true).open(&dp) {
                            let _ = f.set_modified(m);
                        }
                    }
                }
            }
        }
    }
}

/// Map of relative-path -> (size, mtime) for every file under `root`.
fn walk(root: &Path) -> HashMap<String, (u64, SystemTime)> {
    fn go(base: &Path, dir: &Path, out: &mut HashMap<String, (u64, SystemTime)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let name = e.file_name();
            if skipped(&name.to_string_lossy()) {
                continue;
            }
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => go(base, &p, out),
                Ok(_) => {
                    if let (Ok(md), Ok(rel)) = (e.metadata(), p.strip_prefix(base)) {
                        let mt = md.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                        out.insert(rel.to_string_lossy().replace('\\', "/"), (md.len(), mt));
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = HashMap::new();
    go(root, root, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_diff_and_merge_on_synthetic_dirs() {
        let tmp = std::env::temp_dir().join(format!("wse-overlay-{}", std::process::id()));
        let (origin, over) = (tmp.join("origin"), tmp.join("over"));
        // origin: keep.txt, edit.txt, gone.txt (+ a skipped node_modules)
        fs::create_dir_all(origin.join("node_modules")).unwrap();
        fs::write(origin.join("node_modules/huge.bin"), b"x").unwrap();
        fs::write(origin.join("keep.txt"), b"same").unwrap();
        fs::write(origin.join("edit.txt"), b"before").unwrap();
        fs::write(origin.join("gone.txt"), b"remove-me").unwrap();

        // the workspace works on a copy (skips node_modules, preserves mtimes)
        copy_dir(&origin, &over);
        assert!(!over.join("node_modules").exists(), "regenerable trees are skipped");

        // make changes in the overlay: edit, add, delete
        fs::write(over.join("edit.txt"), b"after-longer").unwrap();
        fs::write(over.join("new.txt"), b"brand new").unwrap();
        fs::remove_file(over.join("gone.txt")).unwrap();

        let mut kinds: Vec<_> = diff(&over, &origin).into_iter().map(|c| (c.rel, c.kind)).collect();
        kinds.sort();
        assert_eq!(
            kinds,
            vec![
                ("edit.txt".to_string(), "modified"),
                ("gone.txt".to_string(), "deleted"),
                ("new.txt".to_string(), "added"),
            ]
        );

        // merge back and confirm the host now matches the overlay
        assert_eq!(apply(&over, &origin), 3);
        assert_eq!(fs::read_to_string(origin.join("edit.txt")).unwrap(), "after-longer");
        assert_eq!(fs::read_to_string(origin.join("new.txt")).unwrap(), "brand new");
        assert!(!origin.join("gone.txt").exists());
        assert!(diff(&over, &origin).is_empty(), "after merge, nothing differs");

        let _ = fs::remove_dir_all(&tmp);
    }
}
