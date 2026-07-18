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

export interface ChatMessage {
  id: number;
  mine: boolean;
  text: string;
  time: string;
}

// Direct-message threads, keyed by friend id.
export const conversations: Record<number, ChatMessage[]> = {
  1: [
    { id: 1, mine: false, text: "hey! are you around later?", time: "10:02" },
    { id: 2, mine: true, text: "yeah, after 3 works", time: "10:03" },
    { id: 3, mine: false, text: "perfect. can you lend me a box to test a build?", time: "10:03" },
    { id: 4, mine: true, text: "for sure, i'll spin up a workspace for you", time: "10:04" },
  ],
  2: [
    { id: 1, mine: false, text: "pushed the fix, take a look when you can", time: "09:15" },
    { id: 2, mine: true, text: "on it", time: "09:20" },
  ],
  3: [{ id: 1, mine: true, text: "welcome to Arna :)", time: "yesterday" }],
};

