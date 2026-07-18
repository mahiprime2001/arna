import { useEffect, useMemo, useState } from "react";
import { TitleBar } from "@/components/TitleBar";
import { Sidebar } from "@/components/Sidebar";
import { Dashboard } from "@/views/Dashboard";
import { Workspaces } from "@/views/Workspaces";
import { Friends } from "@/views/Friends";
import { Notifications } from "@/views/Notifications";
import { Profile } from "@/views/Profile";
import { Settings } from "@/views/Settings";
import {
  initialFriends,
  initialNotes,
  initialRequests,
  initialSent,
  user,
  type Friend,
  type FriendRequest,
  type Note,
  type Route,
  type SentRequest,
} from "@/lib/mock";

export type Theme = "dark" | "light";

let nextId = 1000;

export default function App() {
  const [route, setRoute] = useState<Route>("dashboard");
  const [theme, setTheme] = useState<Theme>("dark");
  const [notes, setNotes] = useState<Note[]>(initialNotes);
  const [friends, setFriends] = useState<Friend[]>(initialFriends);
  const [requests, setRequests] = useState<FriendRequest[]>(initialRequests);
  const [sent, setSent] = useState<SentRequest[]>(initialSent);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
  }, [theme]);

  const unread = useMemo(() => notes.filter((n) => !n.read).length, [notes]);

  // Friend actions (mock).
  const acceptRequest = (id: number) => {
    const r = requests.find((x) => x.id === id);
    if (!r) return;
    setFriends((f) => [
      { id: r.id, name: r.name, handle: r.handle, presence: "online" as const },
      ...f,
    ]);
    setRequests((rs) => rs.filter((x) => x.id !== id));
  };
  const declineRequest = (id: number) =>
    setRequests((rs) => rs.filter((x) => x.id !== id));
  const cancelSent = (id: number) =>
    setSent((ss) => ss.filter((x) => x.id !== id));
  const removeFriend = (id: number) =>
    setFriends((f) => f.filter((x) => x.id !== id));
  const addFriend = (handle: string) =>
    setSent((ss) => [{ id: nextId++, handle }, ...ss]);

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-canvas text-ink">
      <TitleBar unread={unread} onBell={() => setRoute("notifications")} />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar
          route={route}
          setRoute={setRoute}
          badges={{ notifications: unread, friends: requests.length }}
          user={user}
        />
        <main className="flex-1 overflow-y-auto">
          <div className="mx-auto max-w-5xl px-8 py-8">
            {route === "dashboard" && (
              <Dashboard
                friends={friends}
                requestCount={requests.length}
                unread={unread}
                setRoute={setRoute}
              />
            )}
            {route === "workspaces" && <Workspaces />}
            {route === "friends" && (
              <Friends
                friends={friends}
                requests={requests}
                sent={sent}
                onAccept={acceptRequest}
                onDecline={declineRequest}
                onCancelSent={cancelSent}
                onRemove={removeFriend}
                onAdd={addFriend}
              />
            )}
            {route === "notifications" && (
              <Notifications notes={notes} setNotes={setNotes} />
            )}
            {route === "profile" && <Profile />}
            {route === "settings" && <Settings theme={theme} setTheme={setTheme} />}
          </div>
        </main>
      </div>
    </div>
  );
}
