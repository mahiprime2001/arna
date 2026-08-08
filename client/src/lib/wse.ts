// Bridge to the native WSE engine embedded in the Tauri backend. In a plain
// browser (no Tauri) these are inert and `isTauri()` is false, so the app keeps
// its mock behaviour; inside the Tauri desktop app they drive real Windows
// workspaces. Every command returns the engine's current workspace list.
import { invoke } from "@tauri-apps/api/core";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  draftWorkspace,
  type Guarantees,
  type Workspace,
  type WorkspaceRole,
  type WorkspaceState,
} from "@/lib/workspace";

/// Open a Docker workspace's code-server (VS Code) in its own app window. Reuses
/// the window if it's already open.
export function openCodeServerWindow(id: string, title: string, url: string) {
  const label = `ws-${id}`;
  WebviewWindow.getByLabel(label).then((existing) => {
    if (existing) {
      existing.setFocus();
    } else {
      new WebviewWindow(label, { url, title: `${title} — VS Code`, width: 1280, height: 820 });
    }
  });
}

/// Turn whatever a friend pasted — a full URL, or a bare `host:port[/path]` —
/// into a clean http URL. Returns null if it isn't a workspace address (e.g. a
/// bare workspace ID, which can't be resolved without a discovery service).
export function normalizeInvite(raw: string): string | null {
  const s = raw.trim();
  if (!s) return null;
  const hasScheme = /^https?:\/\//i.test(s);
  const looksHostPort = /^[\w.-]+:\d{2,5}(\/|$)/.test(s);
  if (!hasScheme && !looksHostPort) return null;
  try {
    const u = new URL(hasScheme ? s : `http://${s}`);
    return u.hostname ? u.toString() : null;
  } catch {
    return null;
  }
}

// ── Invitations (one-click Join) ─────────────────────────────────────────────
// An invite identifies an invitation with a ROLE + how to reach the workspace.
// It is never equivalent to unlimited authority — the workspace decides what a
// role may do. v1 carries the role; enforcement deepens with Watch & Control.
export interface Invite {
  v: 1;
  id: string;
  name: string;
  role: WorkspaceRole;
  url: string;
}

/// Encode an invite as a compact shareable token (base64 of JSON).
export function makeInvite(inv: Invite): string {
  return btoa(unescape(encodeURIComponent(JSON.stringify(inv))));
}

/// Decode an invite token; null if it isn't one.
export function parseInvite(token: string): Invite | null {
  try {
    const o = JSON.parse(decodeURIComponent(escape(atob(token.trim()))));
    if (o && o.v === 1 && typeof o.id === "string" && typeof o.url === "string" && o.role) {
      return o as Invite;
    }
  } catch {
    /* not a token */
  }
  return null;
}

/// Accept either an invite token OR a bare workspace URL. A raw URL becomes a
/// least-privilege Viewer invite for an otherwise-unknown workspace.
export function inviteFrom(input: string): Invite | null {
  const parsed = parseInvite(input);
  if (parsed) return parsed;
  const url = normalizeInvite(input);
  if (!url) return null;
  let h = 0;
  for (let i = 0; i < url.length; i++) h = (h * 31 + url.charCodeAt(i)) | 0;
  return {
    v: 1,
    id: `url-${(h >>> 0).toString(36)}`,
    name: "Shared workspace",
    role: "viewer",
    url,
  };
}

/// Open a joined workspace (its code-server surface) in its own window / tab.
export function openJoined(id: string, name: string, url: string) {
  if (isTauri()) {
    openCodeServerWindow(id, name, url);
  } else {
    window.open(url, "_blank", "noopener");
  }
}

export type EngineWs = {
  id: string;
  name: string;
  runtime: "native" | "docker";
  state: string;
  apps: number;
  url: string | null;
  lanUrl?: string | null;
  guarantees?: Guarantees;
};

export function isTauri(): boolean {
  return typeof (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !==
    "undefined";
}

// Whether a Docker runtime is available (Docker Desktop running). Updated from
// every state reply; the create dialog uses it to offer/withhold the Docker
// runtime — WSE never *requires* Docker.
let _dockerAvailable = false;
export const dockerAvailable = () => _dockerAvailable;

function parse(json: string): EngineWs[] {
  try {
    const o = JSON.parse(json);
    _dockerAvailable = !!o.docker;
    return (o.workspaces ?? []) as EngineWs[];
  } catch {
    return [];
  }
}

async function cmd(name: string, args?: Record<string, unknown>): Promise<EngineWs[]> {
  try {
    return parse(await invoke<string>(name, args));
  } catch {
    return [];
  }
}

export const wseList = () => cmd("ws_list");
export const wseCreate = (name: string, runtime: string, apps: string[]) =>
  cmd("ws_create", { name, runtime, apps });
export const wseStart = (id: string) => cmd("ws_start", { id });
export const wseLaunch = (id: string) => cmd("ws_launch", { id });
export const wseEnter = (id: string) => cmd("ws_enter", { id });
export const wseSuspend = (id: string) => cmd("ws_suspend", { id });
export const wseDestroy = (id: string) => cmd("ws_destroy", { id });
export const wseImport = (id: string, chrome: boolean) => cmd("ws_import", { id, chrome });
export const wseBrowser = (chrome: boolean) => cmd("ws_browser", { chrome });

// ── Overlay Review: the workspace owns its file CHANGES, not your originals ───
export type OverlayChange = { rel: string; kind: "added" | "modified" | "deleted" };

async function invokeJson<T>(name: string, args: Record<string, unknown>, fallback: T): Promise<T> {
  try {
    return JSON.parse(await invoke<string>(name, args)) as T;
  } catch {
    return fallback;
  }
}

export const overlayShare = (id: string, host: string) =>
  invokeJson<{ ok: boolean; name?: string }>("ws_overlay_share", { id, host }, { ok: false });
export const overlayList = (id: string) =>
  invokeJson<{ overlays: string[] }>("ws_overlay_list", { id }, { overlays: [] }).then((r) => r.overlays);
export const overlayChanges = (id: string, name: string) =>
  invokeJson<{ changes: OverlayChange[] }>("ws_overlay_changes", { id, name }, { changes: [] }).then(
    (r) => r.changes,
  );
export const overlayMerge = (id: string, name: string) =>
  invokeJson<{ merged: number }>("ws_overlay_merge", { id, name }, { merged: 0 }).then((r) => r.merged);
export const overlayDiscard = (id: string, name: string) =>
  invokeJson<{ ok: boolean }>("ws_overlay_discard", { id, name }, { ok: false });

const STATE_MAP: Record<string, WorkspaceState> = {
  ready: "created",
  running: "running",
  suspended: "saved",
};

/** Adapt an engine workspace into the UI's richer Workspace shape (defaults for
 *  fields the native engine doesn't model yet). Carries the runtime + code-server
 *  url so the UI can open Docker workspaces in an embedded editor. */
export function toWorkspace(e: EngineWs): Workspace {
  return {
    ...draftWorkspace(),
    id: e.id,
    name: e.name,
    state: STATE_MAP[e.state] ?? "created",
    createdAt: Date.now(),
    runtime: e.runtime,
    url: e.url,
    lanUrl: e.lanUrl ?? null,
    guarantees: e.guarantees,
  } as Workspace;
}

export const toWorkspaces = (list: EngineWs[]): Workspace[] => list.map(toWorkspace);
