// Shared UI types. The social graph is now real (served by the Go backend);
// only chat/workspaces remain local/mock for now. No demo people here.

export type Route =
  | "dashboard"
  | "workspaces"
  | "friends"
  | "messages"
  | "notifications"
  | "profile"
  | "settings";

export type Presence = "online" | "offline" | "workspace";

export interface User {
  name: string;
  email: string;
  handle: string;
  role: string;
}

export interface Friend {
  id: number;
  name: string;
  handle: string;
  presence: Presence;
  pubkey?: string;
}

export interface FriendRequest {
  id: number; // edge id (for accept/decline)
  userId: number;
  name: string;
  handle: string;
}

export interface SentRequest {
  id: number; // edge id (for cancel)
  handle: string;
}

export interface SearchResult {
  id: number;
  name: string;
  handle: string;
  status: "none" | "friends" | "incoming" | "outgoing";
}

export interface Note {
  id: number;
  title: string;
  body: string;
  time: string;
  read: boolean;
}

export type MsgKind = "text" | "image" | "audio" | "file";

export interface ChatMedia {
  data: string; // data: URL (encrypted in transit, local at rest)
  mime: string;
  name?: string;
  size?: number;
  w?: number;
  h?: number;
  dur?: number;
}

export interface ChatMessage {
  id: number; // local render id
  mid: string; // shared id, used for delivery/read receipts
  mine: boolean;
  kind: MsgKind;
  text?: string;
  media?: ChatMedia;
  time: string;
  ts: number;
  status?: "sent" | "delivered" | "read"; // outgoing only

  replyTo?: string; // mid of the quoted message
  replyPreview?: string; // snapshot of the quote, so it survives deletion
  replyAuthor?: string;
  fwdFrom?: string; // original author's name, when forwarded
  edited?: number; // ts of the last edit
  deleted?: boolean; // tombstone: "This message was deleted"
  myReaction?: string; // one emoji each per side, as in a Telegram 1:1 chat
  theirReaction?: string;
  expiresAt?: number; // disappearing messages: ts when this self-destructs
}

/** Per-conversation settings that are ours alone (never sent to the server). */
export interface ThreadMeta {
  ttl?: number; // disappearing-message timer, in seconds. 0/undefined = off
  pinned?: string; // mid of the pinned message
  muted?: boolean;
}

export type Threads = Record<number, ChatMessage[]>;
export type ThreadMetas = Record<number, ThreadMeta>;

/** Preset self-destruct timers, matching Telegram's own choices. */
export const TTL_PRESETS: { label: string; seconds: number }[] = [
  { label: "Off", seconds: 0 },
  { label: "5 seconds", seconds: 5 },
  { label: "1 minute", seconds: 60 },
  { label: "1 hour", seconds: 3600 },
  { label: "1 day", seconds: 86400 },
  { label: "1 week", seconds: 604800 },
];

export const QUICK_REACTIONS = ["👍", "❤️", "🔥", "😂", "😮", "😢"];

// Everything below travels inside the E2E-encrypted envelope. The relay only
// ever sees ciphertext, so every one of these features stays private to the two
// devices -- the server cannot tell an edit from a reaction from a message.
export interface OutgoingPayload {
  kind: MsgKind;
  text?: string;
  media?: ChatMedia;
  replyTo?: string;
  replyPreview?: string;
  replyAuthor?: string;
  fwdFrom?: string;
  ttl?: number; // sender's timer at send time; receiver applies the same
}

export type WirePayload =
  | ({ op?: "msg"; mid: string } & OutgoingPayload)
  | { op: "edit"; mid: string; text: string; ts: number }
  | { op: "delete"; mids: string[] } // delete for everyone
  | { op: "react"; mid: string; emoji: string | null }
  | { op: "typing"; on: boolean }
  | { op: "ttl"; seconds: number }; // "X set messages to disappear after ..."

// Chat is device-local (E2E) and not wired yet; start with no threads.
export const conversations: Record<number, ChatMessage[]> = {};

