// Bridge to the native WSE engine embedded in the Tauri backend. In a plain
// browser (no Tauri) these are inert and `isTauri()` is false, so the app keeps
// its mock behaviour; inside the Tauri desktop app they drive real Windows
// workspaces. Every command returns the engine's current workspace list.
import { invoke } from "@tauri-apps/api/core";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { draftWorkspace, type Workspace, type WorkspaceState } from "@/lib/workspace";

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

export type EngineWs = {
  id: string;
  name: string;
  runtime: "native" | "docker";
  state: string;
  apps: number;
  url: string | null;
  lanUrl?: string | null;
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
  } as Workspace;
}

export const toWorkspaces = (list: EngineWs[]): Workspace[] => list.map(toWorkspace);
