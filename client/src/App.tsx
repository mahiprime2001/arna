import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { TitleBar } from "@/components/TitleBar";
import { Sidebar } from "@/components/Sidebar";
import { CallOverlay } from "@/components/CallOverlay";
import { Dashboard } from "@/views/Dashboard";
import { Workspaces } from "@/views/Workspaces";
import { WorkspaceStage } from "@/components/WorkspaceStage";
import { Friends } from "@/views/Friends";
import { Messages } from "@/views/Messages";
import { Notifications } from "@/views/Notifications";
import { Profile } from "@/views/Profile";
import { Settings } from "@/views/Settings";
import {
  type ChatMessage,
  type Friend,
  type FriendRequest,
  type Note,
  type OutgoingPayload,
  type Route,
  type SentRequest,
  type ThreadMetas,
  type WirePayload,
} from "@/lib/mock";
import * as api from "@/lib/api";
import {
  isTauri,
  wseList,
  wseCreate,
  wseStart,
  wseSuspend,
  wseDestroy,
  wseEnter,
  toWorkspaces,
} from "@/lib/wse";
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
import {
  hhmm,
  loadChats,
  loadMetas,
  metaOf,
  newMid,
  nextMsgId,
  saveChats,
  saveMetas,
  sweepExpired,
  type Threads,
} from "@/lib/chat";
import { callEngine, type CallKind, type CallState } from "@/lib/webrtc";
import {
  canTransition,
  loadWorkspaces,
  newWorkspaceId,
  saveWorkspaces,
  type Workspace,
  type WorkspaceState,
} from "@/lib/workspace";

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
    error: null,
  });

  // Chat (device-local, E2E encrypted over the relay).
  const [chats, setChats] = useState<Threads>(() => loadChats(user.id));
  const [metas, setMetas] = useState<ThreadMetas>(() => loadMetas(user.id));
  const [chatUnread, setChatUnread] = useState<Record<number, number>>({});
  const [openConv, setOpenConv] = useState<number | null>(null);
  const [typing, setTyping] = useState<Record<number, boolean>>({});

  // Workspaces are device-local for now; there is no platform layer to run
  // them yet, so these records are policy, not processes.
  const [workspaces, setWorkspaces] = useState<Workspace[]>(() => loadWorkspaces(user.id));
  const [openWorkspace, setOpenWorkspace] = useState<Workspace | null>(null);
  useEffect(() => {
    saveWorkspaces(user.id, workspaces);
  }, [workspaces, user.id]);

  const openConvRef = useRef<number | null>(null);
  const friendsRef = useRef<Friend[]>(friends);
  const metasRef = useRef<ThreadMetas>(metas);
  const seenRef = useRef<Set<number>>(new Set());
  useEffect(() => {
    metasRef.current = metas;
  }, [metas]);
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
    if (e.type === "signal") {
      // The signal path carries both call setup and live-only chat ops
      // (typing), the latter still encrypted end-to-end.
      if (e.signal?.t === "chat") {
        const fr = friendsRef.current.find((f) => f.id === e.from);
        if (!fr?.pubkey) return;
        const plain = decryptFrom(fr.pubkey, e.signal.nonce, e.signal.ciphertext);
        if (plain != null) applyOp(e.from, JSON.parse(plain) as WirePayload);
        return;
      }
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
    let payload: WirePayload;
    try {
      payload = JSON.parse(plain);
    } catch {
      return;
    }
    applyOp(e.from, payload, e.ts);
  }, []);

  // Apply a decrypted op from the peer. Anything without an `op` is a plain
  // message (which is also what older clients send).
  const applyOp = useCallback((from: number, p: WirePayload, wireTs?: number) => {
    switch (p.op) {
      case "typing":
        setTyping((t) => ({ ...t, [from]: p.on }));
        return;

      case "edit":
        setChats((prev) => {
          const thread = prev[from];
          if (!thread) return prev;
          return {
            ...prev,
            [from]: thread.map((m) => (m.mid === p.mid ? { ...m, text: p.text, edited: p.ts } : m)),
          };
        });
        return;

      case "delete":
        setChats((prev) => {
          const thread = prev[from];
          if (!thread) return prev;
          return { ...prev, [from]: thread.filter((m) => !p.mids.includes(m.mid)) };
        });
        return;

      case "react":
        setChats((prev) => {
          const thread = prev[from];
          if (!thread) return prev;
          return {
            ...prev,
            [from]: thread.map((m) =>
              m.mid === p.mid ? { ...m, theirReaction: p.emoji ?? undefined } : m,
            ),
          };
        });
        return;

      case "ttl":
        // The peer changed the disappearing timer; both sides must agree.
        setMetas((prev) => ({ ...prev, [from]: { ...metaOf(prev, from), ttl: p.seconds } }));
        return;

      default: {
        const ts = wireTs || Date.now();
        const msg: ChatMessage = {
          id: nextMsgId(),
          mid: p.mid,
          mine: false,
          kind: p.kind,
          text: p.text,
          media: p.media,
          replyTo: p.replyTo,
          replyPreview: p.replyPreview,
          // The quote's author is named from the receiver's point of view.
          replyAuthor: p.replyAuthor === "You" ? "Them" : "You",
          fwdFrom: p.fwdFrom,
          time: hhmm(ts),
          ts,
          ...(p.ttl ? { expiresAt: Date.now() + p.ttl * 1000 } : {}),
        };
        setTyping((t) => (t[from] ? { ...t, [from]: false } : t));
        setChats((prev) => ({ ...prev, [from]: [...(prev[from] || []), msg] }));
        sendReceipt(from, "delivered", p.mid);
        if (openConvRef.current === from) {
          sendReceipt(from, "read");
        } else {
          setChatUnread((u) => ({ ...u, [from]: (u[from] || 0) + 1 }));
        }
      }
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
  useEffect(() => {
    saveMetas(user.id, metas);
  }, [metas, user.id]);

  // Disappearing messages: drop anything past its self-destruct time. Runs on a
  // tick so a message vanishes while you are looking at it, as Telegram does.
  useEffect(() => {
    const id = window.setInterval(() => {
      setChats((prev) => sweepExpired(prev) ?? prev);
    }, 1000);
    return () => clearInterval(id);
  }, []);

  // "typing…" is a claim with a short shelf life; expire it if they go quiet.
  useEffect(() => {
    if (!Object.values(typing).some(Boolean)) return;
    const id = window.setTimeout(() => setTyping({}), 4000);
    return () => clearTimeout(id);
  }, [typing]);

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

  // Every chat feature below rides inside the encrypted envelope, so the relay
  // cannot tell an edit from a reaction from a plain message -- it only ever
  // sees ciphertext it must forward.
  const sendOp = (friendId: number, op: WirePayload, live = false) => {
    const fr = friendsRef.current.find((f) => f.id === friendId);
    if (!fr?.pubkey) return;
    const { nonce, ciphertext } = encryptFor(fr.pubkey, JSON.stringify(op));
    // Live ops (typing) go down the signal path, which is never queued for
    // offline delivery -- a stale "typing…" an hour later would be nonsense.
    if (live) sendSignal(friendId, { t: "chat", nonce, ciphertext });
    else sendMsg(friendId, nonce, ciphertext, Date.now());
  };

  const patchThread = (friendId: number, fn: (m: ChatMessage) => ChatMessage | null) =>
    setChats((prev) => {
      const thread = prev[friendId];
      if (!thread) return prev;
      const next = thread.map(fn).filter((m): m is ChatMessage => m !== null);
      return { ...prev, [friendId]: next };
    });

  const sendMessage = (friendId: number, payload: OutgoingPayload) => {
    const ts = Date.now();
    const mid = newMid();
    const ttl = metaOf(metasRef.current, friendId).ttl ?? 0;
    const msg: ChatMessage = {
      id: nextMsgId(),
      mid,
      mine: true,
      kind: payload.kind,
      text: payload.text,
      media: payload.media,
      replyTo: payload.replyTo,
      replyPreview: payload.replyPreview,
      replyAuthor: payload.replyAuthor,
      fwdFrom: payload.fwdFrom,
      time: hhmm(ts),
      ts,
      status: "sent",
      ...(ttl ? { expiresAt: ts + ttl * 1000 } : {}),
    };
    setChats((prev) => ({ ...prev, [friendId]: [...(prev[friendId] || []), msg] }));
    sendOp(friendId, { mid, ...payload, ttl });
  };

  const editMessage = (friendId: number, mid: string, text: string) => {
    const ts = Date.now();
    patchThread(friendId, (m) => (m.mid === mid ? { ...m, text, edited: ts } : m));
    sendOp(friendId, { op: "edit", mid, text, ts });
  };

  const deleteMessage = (friendId: number, m: ChatMessage, forEveryone: boolean) => {
    patchThread(friendId, (x) => (x.mid === m.mid ? null : x));
    if (forEveryone) sendOp(friendId, { op: "delete", mids: [m.mid] });
  };

  const reactToMessage = (friendId: number, m: ChatMessage, emoji: string | null) => {
    patchThread(friendId, (x) =>
      x.mid === m.mid ? { ...x, myReaction: emoji ?? undefined } : x,
    );
    sendOp(friendId, { op: "react", mid: m.mid, emoji });
  };

  const forwardMessage = (toFriendId: number, m: ChatMessage, fromName: string) =>
    sendMessage(toFriendId, {
      kind: m.kind,
      text: m.text,
      media: m.media,
      fwdFrom: m.fwdFrom ?? fromName,
    });

  const setTtl = (friendId: number, seconds: number) => {
    setMetas((prev) => ({ ...prev, [friendId]: { ...metaOf(prev, friendId), ttl: seconds } }));
    sendOp(friendId, { op: "ttl", seconds });
  };

  const toggleMute = (friendId: number) =>
    setMetas((prev) => ({
      ...prev,
      [friendId]: { ...metaOf(prev, friendId), muted: !metaOf(prev, friendId).muted },
    }));

  // Pinning is local: the spec for this app keeps per-device state per-device.
  const pinMessage = (friendId: number, mid: string | null) =>
    setMetas((prev) => ({
      ...prev,
      [friendId]: { ...metaOf(prev, friendId), pinned: mid ?? undefined },
    }));

  const setTypingTo = (friendId: number, on: boolean) =>
    sendOp(friendId, { op: "typing", on }, true);

  // ── workspaces ────────────────────────────────────────────────────────────
  // Under Tauri, workspaces are REAL (the native WSE engine in the Rust backend);
  // in a plain browser they stay device-local mock. Same UI either way.
  const setFromEngine = (list: Awaited<ReturnType<typeof wseList>>) =>
    setWorkspaces(toWorkspaces(list));

  // Keep the list live with the engine while running as the desktop app.
  useEffect(() => {
    if (!isTauri()) return;
    let alive = true;
    const refresh = () => wseList().then((l) => alive && setFromEngine(l));
    refresh();
    const t = setInterval(refresh, 2000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, []);

  const createWorkspace = (draft: Omit<Workspace, "id" | "state" | "createdAt">) => {
    if (isTauri()) {
      // "Chrome" selected in the app picker → use Chrome; else Edge.
      const useChrome = draft.apps?.includes("chrome") ?? false;
      wseCreate(draft.name || "Workspace", useChrome).then(setFromEngine);
      return;
    }
    setWorkspaces((prev) => [
      { ...draft, id: newWorkspaceId(), state: "created", createdAt: Date.now() },
      ...prev,
    ]);
  };

  // Refuse anything the spec's state machine doesn't permit, rather than
  // trusting the UI to only offer legal buttons (SPEC §5.2).
  const transitionWorkspace = (id: string, to: WorkspaceState) => {
    if (isTauri()) {
      const run =
        to === "running" || to === "resuming"
          ? wseStart(id)
          : to === "paused" || to === "saved"
            ? wseSuspend(id)
            : null;
      if (run) run.then(setFromEngine);
      return;
    }
    setWorkspaces((prev) =>
      prev.map((w) => (w.id === id && canTransition(w.state, to) ? { ...w, state: to } : w)),
    );
  };

  // SPEC §5.5: deletion destroys contents, it does not merely unlist them.
  const deleteWorkspace = (w: Workspace) => {
    const wipes = w.persistence === "temporary";
    const ok = window.confirm(
      `Delete "${w.name}"?\n\n${
        wipes
          ? "Everything inside is destroyed for good."
          : "Its saved contents are destroyed for good."
      }\n\nThis cannot be undone.`,
    );
    if (!ok) return;
    if (isTauri()) {
      wseDestroy(w.id).then(setFromEngine);
      return;
    }
    setWorkspaces((prev) => prev.filter((x) => x.id !== w.id));
  };

  // "Open" on the desktop app switches you into the workspace's real desktop
  // (Ctrl+Alt+Q returns); in the browser it opens the in-app stage overlay.
  const handleOpenWorkspace = (w: Workspace) => {
    if (isTauri()) {
      wseEnter(w.id).then(setFromEngine);
      return;
    }
    setOpenWorkspace(w);
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
        <main className="flex-1 overflow-hidden">
          {/* Messages runs full-bleed and full-height the way a chat app should.
              Every other route keeps the centred document column. */}
          {route === "messages" ? (
            <Messages
              friends={friends}
              chats={chats}
              metas={metas}
              unread={chatUnread}
              typing={typing}
              initialFriendId={dmFriend}
              onOpen={openConversation}
              onSend={sendMessage}
              onCall={startCall}
              onTyping={setTypingTo}
              onSetTtl={setTtl}
              onToggleMute={toggleMute}
              onPin={pinMessage}
              onEditMessage={editMessage}
              onDeleteMessage={deleteMessage}
              onReact={reactToMessage}
              onForward={forwardMessage}
            />
          ) : (
            <div className="h-full overflow-y-auto">
              <div className="mx-auto max-w-5xl px-8 py-8">
                {route === "dashboard" && (
                  <Dashboard
                    user={user}
                    friends={friends}
                    requestCount={requests.length}
                    unread={unread}
                    workspaces={workspaces}
                    setRoute={setRoute}
                  />
                )}
                {route === "workspaces" && (
                  <Workspaces
                    workspaces={workspaces}
                    onCreate={createWorkspace}
                    onTransition={transitionWorkspace}
                    onDelete={deleteWorkspace}
                    onOpen={handleOpenWorkspace}
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
            </div>
          )}
        </main>
      </div>

      <CallOverlay state={callState} />

      {openWorkspace && (
        <WorkspaceStage
          workspace={openWorkspace}
          onClose={() => setOpenWorkspace(null)}
          onLaunch={(appId) => api.workspaceLaunch(openWorkspace.id, appId).catch(() => {})}
          onLayout={(layout) => api.workspaceLayout(openWorkspace.id, layout).catch(() => {})}
        />
      )}
    </div>
  );
}
