import { useCallback, useEffect, useMemo, useState } from "react";
import { TitleBar } from "@/components/TitleBar";
import { Sidebar } from "@/components/Sidebar";
import { CallOverlay, type Call } from "@/components/CallOverlay";
import { Dashboard } from "@/views/Dashboard";
import { Workspaces } from "@/views/Workspaces";
import { Friends } from "@/views/Friends";
import { Messages } from "@/views/Messages";
import { Notifications } from "@/views/Notifications";
import { Profile } from "@/views/Profile";
import { Settings } from "@/views/Settings";
import {
  type Friend,
  type FriendRequest,
  type Note,
  type Route,
  type SentRequest,
} from "@/lib/mock";
import * as api from "@/lib/api";
import type { AuthUser } from "@/lib/api";

export type Theme = "dark" | "light";

export default function App({
  user,
  onSignOut,
}: {
  user: AuthUser;
  onSignOut: () => void;
}) {
  const [route, setRoute] = useState<Route>("dashboard");
  const [theme, setTheme] = useState<Theme>("dark");
  const [notes, setNotes] = useState<Note[]>([]);
  const [friends, setFriends] = useState<Friend[]>([]);
  const [requests, setRequests] = useState<FriendRequest[]>([]);
  const [sent, setSent] = useState<SentRequest[]>([]);
  const [dmFriend, setDmFriend] = useState<number | null>(null);
  const [call, setCall] = useState<Call | null>(null);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
  }, [theme]);

  // Live social graph: load, then poll, and heartbeat presence.
  const refresh = useCallback(async () => {
    try {
      const d = await api.getFriends();
      setFriends(d.friends);
      setRequests(d.incoming);
      setSent(d.outgoing);
    } catch {
      // stay with what we have
    }
  }, []);

  useEffect(() => {
    refresh();
    api.ping().catch(() => {});
    const poll = setInterval(refresh, 8000);
    const beat = setInterval(() => api.ping().catch(() => {}), 15000);
    return () => {
      clearInterval(poll);
      clearInterval(beat);
    };
  }, [refresh]);

  const unread = useMemo(() => notes.filter((n) => !n.read).length, [notes]);

  const acceptRequest = async (id: number) => {
    await api.respondFriendRequest(id, "accept");
    refresh();
  };
  const declineRequest = async (id: number) => {
    await api.respondFriendRequest(id, "decline");
    refresh();
  };
  const cancelSent = async (id: number) => {
    await api.cancelFriendRequest(id);
    refresh();
  };
  const removeFriend = async (userId: number) => {
    await api.removeFriend(userId);
    refresh();
  };
  // Throws on failure so the Friends UI can show the server's message.
  const addFriend = async (handle: string) => {
    await api.sendFriendRequest(handle);
    refresh();
  };
  const openDm = (id: number) => {
    setDmFriend(id);
    setRoute("messages");
  };

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
                user={user}
                friends={friends}
                requestCount={requests.length}
                unread={unread}
                setRoute={setRoute}
              />
            )}
            {route === "workspaces" && <Workspaces />}
            {route === "messages" && (
              <Messages friends={friends} initialFriendId={dmFriend} onCall={setCall} />
            )}
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
                onMessage={openDm}
                onCall={setCall}
              />
            )}
            {route === "notifications" && (
              <Notifications notes={notes} setNotes={setNotes} />
            )}
            {route === "profile" && <Profile user={user} onSignOut={onSignOut} />}
            {route === "settings" && <Settings theme={theme} setTheme={setTheme} />}
          </div>
        </main>
      </div>

      {call && <CallOverlay call={call} onEnd={() => setCall(null)} />}
    </div>
  );
}
