// Workspace domain, mirroring docs/SPEC.md. Types and rules only -- nothing
// here knows about WSL2, Windows, or any platform (SPEC §18.4). The options a
// user picks at creation ARE the workspace's policy; nothing is implied.

// ── states ─────────────────────────────────────────────────────────────────
// SPEC §5.1. Exactly one at a time.
export type WorkspaceState =
  | "created"
  | "running"
  | "idle"
  | "paused"
  | "resuming"
  | "saved"
  | "archived"
  | "deleted";

// SPEC §5.2. Anything not listed is forbidden -- this is the whole rule.
const TRANSITIONS: Record<WorkspaceState, WorkspaceState[]> = {
  created: ["running", "deleted"],
  running: ["idle", "paused", "saved", "deleted"],
  idle: ["running", "paused", "saved", "deleted"],
  paused: ["resuming", "saved", "deleted"],
  resuming: ["running"],
  saved: ["resuming", "archived", "deleted"],
  archived: ["saved", "deleted"],
  deleted: [],
};

export const canTransition = (from: WorkspaceState, to: WorkspaceState): boolean =>
  TRANSITIONS[from].includes(to);

export const STATE_LABEL: Record<WorkspaceState, string> = {
  created: "Not started",
  running: "Running",
  idle: "Idle",
  paused: "Paused",
  resuming: "Resuming",
  saved: "Saved",
  archived: "Archived",
  deleted: "Deleted",
};

// ── persistence ────────────────────────────────────────────────────────────
// SPEC §5.4. Both are first-class; neither is a degraded form of the other.
export type Persistence = "temporary" | "saved";

// ── roles and capabilities ─────────────────────────────────────────────────
// SPEC §4. Capabilities are explicit per role, never implied by the role name.
export type Role = "owner" | "collaborator" | "observer";

export type Capability =
  | "viewDisplay"
  | "keyboard"
  | "pointer"
  | "clipboardRead"
  | "clipboardWrite"
  | "fileTransfer";

export const CAPABILITY_LABEL: Record<Capability, string> = {
  viewDisplay: "See the screen",
  keyboard: "Use the keyboard",
  pointer: "Use the mouse",
  clipboardRead: "Copy out of the workspace",
  clipboardWrite: "Paste into the workspace",
  fileTransfer: "Transfer files",
};

/**
 * SPEC §4.6 defaults. "configurable" means the owner may grant it; where a
 * capability is unspecified the answer is no (§6.1), so this table is total.
 * §4.6.1: an Observer is refused everything that takes data OUT, not merely
 * everything that puts input in -- observing is not extracting.
 */
export type Permission = "allowed" | "configurable" | "denied";

export const DEFAULT_CAPABILITIES: Record<Role, Record<Capability, Permission>> = {
  owner: {
    viewDisplay: "allowed",
    keyboard: "allowed",
    pointer: "allowed",
    clipboardRead: "allowed",
    clipboardWrite: "allowed",
    fileTransfer: "allowed",
  },
  collaborator: {
    viewDisplay: "allowed",
    keyboard: "configurable",
    pointer: "configurable",
    clipboardRead: "configurable",
    clipboardWrite: "configurable",
    fileTransfer: "configurable",
  },
  observer: {
    viewDisplay: "allowed",
    keyboard: "denied",
    pointer: "denied",
    clipboardRead: "denied",
    clipboardWrite: "denied",
    fileTransfer: "denied",
  },
};

// ── grants ─────────────────────────────────────────────────────────────────
// SPEC §6.2: explicit, scoped, revocable. §8.5: read-only or read-write.
export interface PathGrant {
  id: string;
  path: string;
  access: "ro" | "rw";
}

// ── resources ──────────────────────────────────────────────────────────────
// SPEC §7.4. Undefined means "no limit set by the host".
export interface ResourceLimits {
  cpuCores?: number;
  memoryGb?: number;
  storageGb?: number;
}

// ── applications ───────────────────────────────────────────────────────────
// SPEC §7.1 / §10.1: applications come from a host-curated catalogue.
export interface CatalogApp {
  id: string;
  name: string;
  hint: string;
}

export const CATALOG: CatalogApp[] = [
  { id: "vscode", name: "VS Code", hint: "Editor, terminal, extensions" },
  { id: "chrome", name: "Chrome", hint: "Browser with its own profile" },
  { id: "terminal", name: "Terminal", hint: "A shell inside the workspace" },
  { id: "files", name: "Files", hint: "File Explorer at the workspace folder" },
  { id: "notepad", name: "Notepad", hint: "A quick text editor" },
];

// A friendly random name so creating a workspace needs no typing (adjective-noun,
// like "swift-otter"). The user can still rename it.
const NAME_ADJ = ["swift", "calm", "brave", "bright", "keen", "bold", "quiet", "lucky", "clever", "cosmic"];
const NAME_NOUN = ["otter", "falcon", "cedar", "harbor", "comet", "willow", "ember", "delta", "atlas", "pine"];
export function randomWorkspaceName(): string {
  const pick = (a: string[]) => a[Math.floor(Math.random() * a.length)];
  return `${pick(NAME_ADJ)}-${pick(NAME_NOUN)}`;
}

// ── the workspace ──────────────────────────────────────────────────────────
export interface Workspace {
  id: string;
  name: string;
  state: WorkspaceState;
  persistence: Persistence;
  apps: string[]; // CatalogApp ids
  shares: PathGrant[];
  limits: ResourceLimits;
  /** SPEC §13.1: internet is on by default. Local network is ALWAYS blocked
   *  (§13.2) -- that is isolation, not a setting, so it is not represented. */
  internet: boolean;
  /** SPEC §16.3: host-configurable behaviour when nobody is connected. */
  whenEmpty: "pause" | "keep-running";
  /** SPEC §4.6.2: what a Collaborator may do, within what the host allows. */
  collaboratorGrants: Record<Capability, boolean>;
  createdAt: number;
  /** Which runtime runs this workspace (native Windows, a Docker sandbox, or —
   *  later — a cloud VM). */
  runtime?: "native" | "docker" | "cloud";
  /** For Docker workspaces: the embedded code-server URL (when running). */
  url?: string | null;
  /** For Docker workspaces: the LAN URL a second machine on the same network
   *  opens to reach this workspace (this host's IP + the mapped port). */
  lanUrl?: string | null;
  /** The runtime's honest guarantees per resource aspect (WRM). Rendered as-is;
   *  the UI never needs to know which runtime produced them. */
  guarantees?: Guarantees;
}

/** How strongly a runtime enforces a projection (WRM `Guarantee`). */
export type GuaranteeStrength = "strong" | "isolated" | "shared" | "partial" | "unsupported";

export interface Guarantees {
  environment: GuaranteeStrength;
  workingDirectory: GuaranteeStrength;
  overlay: GuaranteeStrength;
  processTree: GuaranteeStrength;
  clipboard: GuaranteeStrength;
  registry: GuaranteeStrength;
  network: GuaranteeStrength;
}

/**
 * SPEC §3.3: identifiers are shown to humans in invitations, so they MUST be
 * unguessable and MUST NOT be sequential. 128 bits of randomness, grouped for
 * reading aloud.
 */
export function newWorkspaceId(): string {
  const b = new Uint8Array(16);
  crypto.getRandomValues(b);
  const hex = Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 16)}-${hex.slice(16, 24)}-${hex.slice(24)}`;
}

/** A new workspace's defaults are deny-by-default (§6.1) except where the spec
 *  says otherwise: internet on (§13.1), view-display allowed (§4.6). */
export function draftWorkspace(): Omit<Workspace, "id" | "state" | "createdAt"> {
  return {
    name: randomWorkspaceName(),
    runtime: "native",
    persistence: "saved",
    apps: ["vscode", "chrome"],
    shares: [],
    limits: {},
    internet: true,
    whenEmpty: "pause",
    collaboratorGrants: {
      viewDisplay: true,
      keyboard: true,
      pointer: true,
      clipboardRead: false,
      clipboardWrite: false,
      fileTransfer: false,
    },
  };
}

// ── membership: workspaces someone shared with you (NOT ownership) ───────────
// Ownership ("workspaces I own", above) and membership ("workspaces I may enter")
// are deliberately separate concepts. An invite grants membership at a role; the
// workspace — not the link — decides what that role may do (enforcement grows with
// Watch & Control). SPEC §4 roles map onto the Authorizer already in the engine.
export type WorkspaceRole = "viewer" | "collaborator" | "controller";

export const ROLE_LABEL: Record<WorkspaceRole, string> = {
  viewer: "Viewer",
  collaborator: "Collaborator",
  controller: "Controller",
};

export const ROLE_HINT: Record<WorkspaceRole, string> = {
  viewer: "Can see the workspace",
  collaborator: "Can work alongside the owner",
  controller: "Can take control of the workspace",
};

/** A workspace someone else shared with you (membership, device-local). */
export interface JoinedWorkspace {
  id: string;
  name: string;
  role: WorkspaceRole;
  url: string;
  joinedAt: number;
}

const JOINED_KEY = (uid: number) => `arna_joined_${uid}`;

export function loadJoined(uid: number): JoinedWorkspace[] {
  try {
    return JSON.parse(localStorage.getItem(JOINED_KEY(uid)) || "[]");
  } catch {
    return [];
  }
}

export function saveJoined(uid: number, list: JoinedWorkspace[]) {
  try {
    localStorage.setItem(JOINED_KEY(uid), JSON.stringify(list));
  } catch {
    /* quota; ignore */
  }
}

// ── device-local store (no server yet) ──────────────────────────────────────
const KEY = (uid: number) => `arna_workspaces_${uid}`;

export function loadWorkspaces(uid: number): Workspace[] {
  try {
    return JSON.parse(localStorage.getItem(KEY(uid)) || "[]");
  } catch {
    return [];
  }
}

export function saveWorkspaces(uid: number, list: Workspace[]) {
  try {
    localStorage.setItem(KEY(uid), JSON.stringify(list));
  } catch {
    /* quota; ignore */
  }
}

/** Short summary for the workspace card. */
export function describe(w: Workspace): string {
  const bits = [
    w.persistence === "temporary" ? "Temporary" : "Saved",
    `${w.apps.length} app${w.apps.length === 1 ? "" : "s"}`,
  ];
  if (w.shares.length) bits.push(`${w.shares.length} shared folder${w.shares.length === 1 ? "" : "s"}`);
  if (!w.internet) bits.push("No internet");
  return bits.join(" · ");
}
