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

export interface ChatMessage {
  id: number;
  mine: boolean;
  text: string;
  time: string;
}

// Chat is device-local (E2E) and not wired yet; start with no threads.
export const conversations: Record<number, ChatMessage[]> = {};

// No workspaces yet, by design.
export const workspaces: { id: string; name: string; state: string }[] = [];
