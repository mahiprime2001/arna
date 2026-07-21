import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
import { decryptFrom, encryptFor, initCrypto, myPublicKey } from "@/lib/crypto";
import { connectWs, disconnectWs, sendWs, type Envelope } from "@/lib/ws";
import { hhmm, loadChats, nextMsgId, saveChats, type Threads } from "@/lib/chat";

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

  // Chat (device-local, E2E encrypted over the relay).
  const [chats, setChats] = useState<Threads>(() => loadChats(user.id));
  const [chatUnread, setChatUnread] = useState<Record<number, number>>({});
  const [openConv, setOpenConv] = useState<number | null>(null);

  const openConvRef = useRef<number | null>(null);
  const friendsRef = useRef<Friend[]>(friends);
  const seenRef = useRef<Set<number>>(new Set());
  useEffect(() => {
    openConvRef.current = openConv;
  }, [openConv]);
  useEffect(() => {
    friendsRef.current = friends;
  }, [friends]);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
  }, [theme]);

  // Live social graph.
  const refresh = useCallback(async () => {
    try {
      const d = await api.getFriends();
      setFriends(d.friends);
      setRequests(d.incoming);
      setSent(d.outgoing);
    } catch {
      /* keep current */
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

  // Incoming encrypted message: decrypt with the sender's public key, store,
  // and bump unread unless that conversation is open.
  const onIncoming = useCallback((env: Envelope) => {
    if (seenRef.current.has(env.id)) return; // dedupe
    seenRef.current.add(env.id);
    const fr = friendsRef.current.find((f) => f.id === env.from);
    if (!fr?.pubkey) return;
    const text = decryptFrom(fr.pubkey, env.nonce, env.ciphertext);
    if (text == null) return;
    const msg = { id: nextMsgId(), mine: false, text, time: hhmm(env.ts || Date.now()) };
    setChats((prev) => ({ ...prev, [env.from]: [...(prev[env.from] || []), msg] }));
    if (openConvRef.current !== env.from) {
      setChatUnread((u) => ({ ...u, [env.from]: (u[env.from] || 0) + 1 }));
    }
  }, []);

  // Publish our public key, open the relay.
  useEffect(() => {
    initCrypto(user.id);
    api.setPubkey(myPublicKey()).catch(() => {});
    connectWs(onIncoming);
    return () => disconnectWs();
  }, [user.id, onIncoming]);

  // Persist chat locally.
  useEffect(() => {
    saveChats(user.id, chats);
  }, [chats, user.id]);

  // Leaving Messages closes the active conversation.
  useEffect(() => {
    if (route !== "messages") setOpenConv(null);
  }, [route]);

  const unread = useMemo(() => notes.filter((n) => !n.read).length, [notes]);
  const totalChatUnread = useMemo(
    () => Object.values(chatUnread).reduce((a, b) => a + b, 0),
    [chatUnread],
  );

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
  const addFriend = async (handle: string) => {
    await api.sendFriendRequest(handle);
    refresh();
  };
  const openDm = (id: number) => {
    setDmFriend(id);
    setRoute("messages");
  };

  const sendMessage = (friendId: number, text: string) => {
    const fr = friends.find((f) => f.id === friendId);
    const ts = Date.now();
    const msg = { id: nextMsgId(), mine: true, text, time: hhmm(ts) };
    setChats((prev) => ({ ...prev, [friendId]: [...(prev[friendId] || []), msg] }));
    if (fr?.pubkey) {
      const { nonce, ciphertext } = encryptFor(fr.pubkey, text);
      sendWs(friendId, nonce, ciphertext, ts);
    }
  };

  const openConversation = (friendId: number) => {
    setOpenConv(friendId);
    setChatUnread((u) => {
      const n = { ...u };
      delete n[friendId];
      return n;
    });
  };

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-canvas text-ink">
      <TitleBar unread={unread} onBell={() => setRoute("notifications")} />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar
          route={route}
          setRoute={setRoute}
          badges={{
            notifications: unread,
            friends: requests.length,
            messages: totalChatUnread,
          }}
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
              <Messages
                friends={friends}
                chats={chats}
                unread={chatUnread}
                initialFriendId={dmFriend}
                onOpen={openConversation}
                onSend={sendMessage}
                onCall={setCall}
              />
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
