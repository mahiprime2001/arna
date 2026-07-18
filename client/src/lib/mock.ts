// Mock data only. No backend, no persistence. Stand-ins so the shell renders.

export type Route =
  | "dashboard"
  | "workspaces"
  | "friends"
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
  id: number;
  name: string;
  handle: string;
  mutual: number;
}

export interface SentRequest {
  id: number;
  handle: string;
}

export interface Note {
  id: number;
  title: string;
  body: string;
  time: string;
  read: boolean;
}

export const user: User = {
  name: "Tarun Matta",
  email: "tarun@arna.dev",
  handle: "@tarun",
  role: "Host",
};

export const initialFriends: Friend[] = [
  { id: 1, name: "Aisha Rahman", handle: "@aisha", presence: "online" },
  { id: 2, name: "Marco Silva", handle: "@marco", presence: "workspace" },
  { id: 3, name: "Lena Fischer", handle: "@lena", presence: "online" },
  { id: 4, name: "Devan Rao", handle: "@devan", presence: "offline" },
];

export const initialRequests: FriendRequest[] = [
  { id: 11, name: "Priya Nair", handle: "@priya", mutual: 3 },
  { id: 12, name: "Sam Okafor", handle: "@sam", mutual: 1 },
];

export const initialSent: SentRequest[] = [{ id: 21, handle: "@noor" }];

export const initialNotes: Note[] = [
  {
    id: 1,
    title: "Welcome to Arna",
    body: "Your platform is ready. Create your first workspace to lend some compute.",
    time: "just now",
    read: false,
  },
  {
    id: 2,
    title: "Priya wants to connect",
    body: "You have a new friend request waiting in Friends.",
    time: "2h ago",
    read: false,
  },
  {
    id: 3,
    title: "Tip",
    body: "You can switch to Light mode from Settings, Appearance.",
    time: "yesterday",
    read: true,
  },
];

// No workspaces yet, by design.
export const workspaces: { id: string; name: string; state: string }[] = [];
