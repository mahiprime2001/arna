// Device-local chat history. Threads are kept per-account in localStorage.
// (At-rest encryption + backup to the user's own cloud is a later refinement.)
import type { ChatMessage } from "./mock";

export type Threads = Record<number, ChatMessage[]>;

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

let counter = Date.now();
export const nextMsgId = () => counter++;

export function hhmm(ts: number): string {
  const d = new Date(ts);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}
