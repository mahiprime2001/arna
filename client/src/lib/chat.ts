// Device-local chat history. Threads are kept per-account in localStorage.
// (At-rest encryption + backup to the user's own cloud is a later refinement.)
import type { ChatMessage, ThreadMeta, Threads, ThreadMetas } from "./mock";

export type { Threads, ThreadMetas };

export function loadChats(uid: number): Threads {
  try {
    return JSON.parse(localStorage.getItem(`arna_chat_${uid}`) || "{}");
  } catch {
    return {};
  }
}

export function saveChats(uid: number, threads: Threads) {
  try {
    localStorage.setItem(`arna_chat_${uid}`, JSON.stringify(threads));
  } catch {
    /* quota; ignore for now */
  }
}

export function loadMetas(uid: number): ThreadMetas {
  try {
    return JSON.parse(localStorage.getItem(`arna_chatmeta_${uid}`) || "{}");
  } catch {
    return {};
  }
}

export function saveMetas(uid: number, metas: ThreadMetas) {
  try {
    localStorage.setItem(`arna_chatmeta_${uid}`, JSON.stringify(metas));
  } catch {
    /* quota; ignore */
  }
}

export const metaOf = (metas: ThreadMetas, id: number): ThreadMeta => metas[id] ?? {};

let counter = Date.now();
export const nextMsgId = () => counter++;

/** Short random id shared by both devices, used for receipts/edits/reactions. */
export function newMid(): string {
  const b = new Uint8Array(9);
  crypto.getRandomValues(b);
  return Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
}

export function hhmm(ts: number): string {
  const d = new Date(ts);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** Day separator label, as Telegram shows above the first message of each day. */
export function dayLabel(ts: number): string {
  const d = new Date(ts);
  const today = new Date();
  const y = new Date();
  y.setDate(today.getDate() - 1);
  const same = (a: Date, b: Date) => a.toDateString() === b.toDateString();
  if (same(d, today)) return "Today";
  if (same(d, y)) return "Yesterday";
  return d.toLocaleDateString(undefined, { day: "numeric", month: "long" });
}

/** Human countdown for a disappearing message ("12s", "4m", "2h", "3d"). */
export function untilLabel(expiresAt: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((expiresAt - now) / 1000));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.round(s / 60)}m`;
  if (s < 86400) return `${Math.round(s / 3600)}h`;
  return `${Math.round(s / 86400)}d`;
}

/**
 * Drop every message whose self-destruct time has passed. Returns null when
 * nothing expired, so callers can skip a re-render and a localStorage write.
 */
export function sweepExpired(threads: Threads, now = Date.now()): Threads | null {
  let changed = false;
  const out: Threads = {};
  for (const [k, list] of Object.entries(threads)) {
    const kept = list.filter((m) => !(m.expiresAt && m.expiresAt <= now));
    if (kept.length !== list.length) changed = true;
    out[Number(k)] = kept;
  }
  return changed ? out : null;
}

/** One-line summary used by reply quotes, the pinned bar, and forwards. */
export function summarize(m: ChatMessage): string {
  if (m.deleted) return "Deleted message";
  if (m.kind === "image") return "Photo";
  if (m.kind === "audio") return "Voice message";
  if (m.kind === "file") return m.media?.name || "File";
  return m.text || "";
}
