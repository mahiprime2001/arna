// Mock data only. No backend, no persistence. Stand-ins so the shell renders.

export type Route =
  | "dashboard"
  | "workspaces"
  | "notifications"
  | "profile"
  | "settings";

export interface User {
  name: string;
  email: string;
  role: string;
}

export interface Note {
  id: number;
  title: string;
  body: string;
  time: string;
  read: boolean;
}

export interface Friend {
  name: string;
  online: boolean;
}

export interface Workspace {
  id: string;
  name: string;
  state: string;
}

export const user: User = {
  name: "Tarun Matta",
  email: "tarun@arna.dev",
  role: "Host",
};

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
    title: "Friend request",
    body: "Aisha wants to connect with you.",
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

export const friends: Friend[] = [
  { name: "Aisha", online: true },
  { name: "Marco", online: true },
  { name: "Devan", online: false },
];

// No workspaces yet, by design.
export const workspaces: Workspace[] = [];
