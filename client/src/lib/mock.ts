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
}

export interface OutgoingPayload {
  kind: MsgKind;
  text?: string;
  media?: ChatMedia;
}

// Chat is device-local (E2E) and not wired yet; start with no threads.
export const conversations: Record<number, ChatMessage[]> = {};

// No workspaces yet, by design.
export const workspaces: { id: string; name: string; state: string }[] = [];
