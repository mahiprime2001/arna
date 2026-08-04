// Bridge to the native WSE engine embedded in the Tauri backend. In a plain
// browser (no Tauri) these are inert and `isTauri()` is false, so the app keeps
// its mock behaviour; inside the Tauri desktop app they drive real Windows
// workspaces. Every command returns the engine's current workspace list.
import { invoke } from "@tauri-apps/api/core";
import { draftWorkspace, type Workspace, type WorkspaceState } from "@/lib/workspace";

export type EngineWs = { id: string; name: string; state: string; apps: number };

export function isTauri(): boolean {
  return typeof (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !==
    "undefined";
}

function parse(json: string): EngineWs[] {
  try {
    return (JSON.parse(json).workspaces ?? []) as EngineWs[];
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
export const wseCreate = (name: string, apps: string[]) => cmd("ws_create", { name, apps });
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
 *  fields the native engine doesn't model yet). */
export function toWorkspace(e: EngineWs): Workspace {
  return {
    ...draftWorkspace(),
    id: e.id,
    name: e.name,
    state: STATE_MAP[e.state] ?? "created",
    createdAt: Date.now(),
  } as Workspace;
}

export const toWorkspaces = (list: EngineWs[]): Workspace[] => list.map(toWorkspace);
