import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { TitleBar } from "@/components/TitleBar";
import { Sidebar } from "@/components/Sidebar";
import { CallOverlay } from "@/components/CallOverlay";
import { Dashboard } from "@/views/Dashboard";
import { Workspaces } from "@/views/Workspaces";
import { Friends } from "@/views/Friends";
import { Messages } from "@/views/Messages";
import { Notifications } from "@/views/Notifications";
import { Profile } from "@/views/Profile";
import { Settings } from "@/views/Settings";
import {
  type ChatMedia,
  type ChatMessage,
  type Friend,
  type FriendRequest,
  type MsgKind,
  type Note,
  type OutgoingPayload,
  type Route,
  type SentRequest,
} from "@/lib/mock";
import * as api from "@/lib/api";
import type { AuthUser } from "@/lib/api";
import { decryptFrom, encryptFor, initCrypto, myPublicKey } from "@/lib/crypto";
import {
  connectWs,
  disconnectWs,
  sendMsg,
  sendReceipt,
  sendSignal,
  type Incoming,
} from "@/lib/ws";
import { hhmm, loadChats, nextMsgId, saveChats, type Threads } from "@/lib/chat";
import { callEngine, type CallKind, type CallState } from "@/lib/webrtc";

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
  const [callState, setCallState] = useState<CallState>({
    status: "idle",
    peerId: null,
    peerName: "",
    kind: "audio",
    muted: false,
    camOff: false,
    localStream: null,
    remoteStream: null,
  });

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
  const onIncoming = useCallback((e: Incoming) => {
    // Call signaling (offer/answer/ice/end).
    if (e.type === "signal") {
      callEngine.onSignal(e.from, e.signal);
      return;
    }
    // Receipt: update our sent messages' status (delivered/read).
    if (e.type === "receipt") {
      setChats((prev) => {
        const thread = prev[e.from];
        if (!thread) return prev;
        let changed = false;
        const next = thread.map((m) => {
          if (!m.mine) return m;
          if (e.receipt === "read" && m.status !== "read") {
            changed = true;
            return { ...m, status: "read" as const };
          }
          if (e.receipt === "delivered" && m.mid === e.mid && m.status === "sent") {
            changed = true;
            return { ...m, status: "delivered" as const };
          }
          return m;
        });
        return changed ? { ...prev, [e.from]: next } : prev;
      });
      return;
    }

    // Message: dedupe, decrypt, store, acknowledge.
    if (seenRef.current.has(e.id)) return;
    seenRef.current.add(e.id);
    const fr = friendsRef.current.find((f) => f.id === e.from);
    if (!fr?.pubkey) return;
    const plain = decryptFrom(fr.pubkey, e.nonce, e.ciphertext);
    if (plain == null) return;
    let payload: { mid: string; kind: MsgKind; text?: string; media?: ChatMedia };
    try {
      payload = JSON.parse(plain);
    } catch {
      return;
    }
    const ts = e.ts || Date.now();
    const msg: ChatMessage = {
      id: nextMsgId(),
      mid: payload.mid,
      mine: false,
      kind: payload.kind,
      text: payload.text,
      media: payload.media,
      time: hhmm(ts),
      ts,
    };
    setChats((prev) => ({ ...prev, [e.from]: [...(prev[e.from] || []), msg] }));
    sendReceipt(e.from, "delivered", payload.mid);
    if (openConvRef.current === e.from) {
      sendReceipt(e.from, "read");
    } else {
      setChatUnread((u) => ({ ...u, [e.from]: (u[e.from] || 0) + 1 }));
    }
  }, []);

  // Publish our public key, open the relay.
  useEffect(() => {
    initCrypto(user.id);
    api.setPubkey(myPublicKey()).catch(() => {});
    connectWs(onIncoming);
    return () => disconnectWs();
  }, [user.id, onIncoming]);

  // Wire the call engine to the relay + UI.
  useEffect(() => {
    callEngine.setSignaler(sendSignal);
    callEngine.setListener(setCallState);
    callEngine.setResolveName(
      (id) => friendsRef.current.find((f) => f.id === id)?.name ?? "Unknown",
    );
  }, []);

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

  const sendMessage = (friendId: number, payload: OutgoingPayload) => {
    const fr = friends.find((f) => f.id === friendId);
    const ts = Date.now();
    const mid = crypto.randomUUID();
    const msg: ChatMessage = {
      id: nextMsgId(),
      mid,
      mine: true,
      kind: payload.kind,
      text: payload.text,
      media: payload.media,
      time: hhmm(ts),
      ts,
      status: "sent",
    };
    setChats((prev) => ({ ...prev, [friendId]: [...(prev[friendId] || []), msg] }));
    if (fr?.pubkey) {
      const wire = JSON.stringify({
        mid,
        kind: payload.kind,
        text: payload.text,
        media: payload.media,
      });
      const { nonce, ciphertext } = encryptFor(fr.pubkey, wire);
      sendMsg(friendId, nonce, ciphertext, ts);
    }
  };

  const startCall = (peerId: number, name: string, kind: CallKind) =>
    callEngine.start(peerId, name, kind);

  const openConversation = (friendId: number) => {
    setOpenConv(friendId);
    setChatUnread((u) => {
      const n = { ...u };
      delete n[friendId];
      return n;
    });
    sendReceipt(friendId, "read"); // tell them we've read their messages
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
                onCall={startCall}
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
                onCall={startCall}
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

      <CallOverlay state={callState} />
    </div>
  );
}
