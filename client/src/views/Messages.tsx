import { useEffect, useMemo, useState } from "react";
import {
  Phone,
  VideoCamera,
  MagnifyingGlass,
  Timer,
  BellSlash,
  Bell,
  X,
  ArrowBendUpRight,
} from "@phosphor-icons/react";
import { Avatar } from "@/components/Avatar";
import { Chat, type MsgActions } from "@/components/Chat";
import { cn } from "@/lib/utils";
import { summarize, metaOf } from "@/lib/chat";
import {
  TTL_PRESETS,
  type ChatMessage,
  type Friend,
  type OutgoingPayload,
  type Presence,
  type ThreadMetas,
  type Threads,
} from "@/lib/mock";
import type { CallKind } from "@/lib/webrtc";

function previewOf(msgs?: ChatMessage[]): string {
  if (!msgs || !msgs.length) return "No messages yet";
  return summarize(msgs[msgs.length - 1]);
}

const dot: Record<Presence, string> = {
  online: "bg-good",
  workspace: "bg-warn",
  offline: "bg-muted/50",
};

const ttlLabel = (s?: number) =>
  s ? (TTL_PRESETS.find((p) => p.seconds === s)?.label ?? `${s}s`) : "Off";

export function Messages({
  friends,
  chats,
  metas,
  unread,
  typing,
  initialFriendId,
  onOpen,
  onSend,
  onCall,
  onTyping,
  onSetTtl,
  onToggleMute,
  onPin,
  onEditMessage,
  onDeleteMessage,
  onReact,
  onForward,
}: {
  friends: Friend[];
  chats: Threads;
  metas: ThreadMetas;
  unread: Record<number, number>;
  typing: Record<number, boolean>;
  initialFriendId: number | null;
  onOpen: (friendId: number) => void;
  onSend: (friendId: number, payload: OutgoingPayload) => void;
  onCall: (peerId: number, name: string, kind: CallKind) => void;
  onTyping: (friendId: number, on: boolean) => void;
  onSetTtl: (friendId: number, seconds: number) => void;
  onToggleMute: (friendId: number) => void;
  onPin: (friendId: number, mid: string | null) => void;
  onEditMessage: (friendId: number, mid: string, text: string) => void;
  onDeleteMessage: (friendId: number, m: ChatMessage, forEveryone: boolean) => void;
  onReact: (friendId: number, m: ChatMessage, emoji: string | null) => void;
  onForward: (toFriendId: number, m: ChatMessage, fromName: string) => void;
}) {
  const [selectedId, setSelectedId] = useState<number | null>(
    initialFriendId ?? friends[0]?.id ?? null,
  );
  const [search, setSearch] = useState("");
  const [searching, setSearching] = useState(false);
  const [ttlOpen, setTtlOpen] = useState(false);
  const [forwarding, setForwarding] = useState<ChatMessage | null>(null);

  // Opening a conversation clears its unread badge (in the parent).
  useEffect(() => {
    if (selectedId != null) onOpen(selectedId);
  }, [selectedId, onOpen]);

  // Search and the timer menu are per-conversation.
  useEffect(() => {
    setSearch("");
    setSearching(false);
    setTtlOpen(false);
  }, [selectedId]);

  const selected = friends.find((f) => f.id === selectedId) ?? null;
  const messages = selectedId != null ? (chats[selectedId] ?? []) : [];
  const meta = selectedId != null ? metaOf(metas, selectedId) : {};

  const preview = useMemo(() => {
    const p: Record<number, string> = {};
    for (const f of friends) p[f.id] = previewOf(chats[f.id]);
    return p;
  }, [friends, chats]);

  const actions: MsgActions | null = selected
    ? {
        onReply: () => {}, // handled inside Chat
        onEdit: () => {}, // handled inside Chat
        onForward: (m) => setForwarding(m),
        onPin: (m) => onPin(selected.id, m.mid),
        onDelete: (m, forEveryone) => onDeleteMessage(selected.id, m, forEveryone),
        onReact: (m, emoji) => onReact(selected.id, m, emoji),
      }
    : null;

  return (
    <div className="flex h-full overflow-hidden bg-surface">
      {/* Conversation list */}
      <div className="flex w-72 shrink-0 flex-col border-r border-line">
        <div className="border-b border-line px-4 py-3">
          <h1 className="text-base font-semibold">Messages</h1>
        </div>
        <div className="flex-1 overflow-y-auto p-2">
          {friends.length === 0 && (
            <p className="px-3 py-6 text-center text-[13px] text-muted">
              Add a friend to start chatting.
            </p>
          )}
          {friends.map((f) => {
            const fm = metaOf(metas, f.id);
            return (
              <button
                key={f.id}
                onClick={() => setSelectedId(f.id)}
                className={cn(
                  "flex w-full items-center gap-3 rounded-lg px-2.5 py-2 text-left transition-colors",
                  selectedId === f.id ? "bg-brand-soft" : "hover:bg-elevated",
                )}
              >
                <div className="relative">
                  <Avatar name={f.name} size={38} />
                  <span
                    className={cn(
                      "absolute -bottom-0.5 -right-0.5 h-3 w-3 rounded-full border-2 border-surface",
                      dot[f.presence],
                    )}
                  />
                </div>
                <div className="min-w-0 flex-1">
                  <p className="flex items-center gap-1 truncate text-sm font-medium">
                    {f.name}
                    {fm.ttl ? <Timer size={12} className="shrink-0 text-brand" /> : null}
                    {fm.muted ? <BellSlash size={12} className="shrink-0 text-muted" /> : null}
                  </p>
                  <p className="truncate text-[12.5px] text-muted">
                    {typing[f.id] ? <span className="italic text-brand">typing…</span> : preview[f.id]}
                  </p>
                </div>
                {unread[f.id] > 0 && (
                  <span
                    className={cn(
                      "grid h-5 min-w-[20px] place-items-center rounded-full px-1.5 text-[11px] font-semibold",
                      fm.muted ? "bg-muted/40 text-ink" : "bg-brand text-brand-fg",
                    )}
                  >
                    {unread[f.id]}
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </div>

      {/* Thread */}
      {selected && actions ? (
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="relative flex items-center gap-3 border-b border-line px-4 py-2.5">
            <Avatar name={selected.name} size={34} />
            <div className="min-w-0">
              <p className="text-sm font-medium leading-tight">{selected.name}</p>
              <p className="text-[12px] text-muted">
                {typing[selected.id] ? (
                  <span className="text-brand">typing…</span>
                ) : selected.presence === "workspace" ? (
                  "In a workspace"
                ) : (
                  <span className="capitalize">{selected.presence}</span>
                )}
              </p>
            </div>

            <div className="ml-auto flex items-center gap-1">
              {searching ? (
                <div className="flex items-center gap-1.5 rounded-lg border border-line bg-canvas px-2.5">
                  <MagnifyingGlass size={15} className="text-muted" />
                  <input
                    autoFocus
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                    placeholder="Search messages"
                    className="h-8 w-44 bg-transparent text-sm outline-none placeholder:text-muted/70"
                  />
                  <button
                    onClick={() => {
                      setSearch("");
                      setSearching(false);
                    }}
                    aria-label="Close search"
                    className="text-muted hover:text-ink"
                  >
                    <X size={14} />
                  </button>
                </div>
              ) : (
                <button
                  onClick={() => setSearching(true)}
                  aria-label="Search messages"
                  className="grid h-9 w-9 place-items-center rounded-lg text-muted transition-colors hover:bg-elevated hover:text-ink"
                >
                  <MagnifyingGlass size={18} />
                </button>
              )}

              <button
                onClick={() => setTtlOpen((v) => !v)}
                aria-label="Disappearing messages"
                title={`Disappearing messages: ${ttlLabel(meta.ttl)}`}
                className={cn(
                  "grid h-9 w-9 place-items-center rounded-lg transition-colors hover:bg-elevated",
                  meta.ttl ? "text-brand" : "text-muted hover:text-ink",
                )}
              >
                <Timer size={18} weight={meta.ttl ? "fill" : "regular"} />
              </button>

              <button
                onClick={() => onToggleMute(selected.id)}
                aria-label={meta.muted ? "Unmute" : "Mute"}
                className="grid h-9 w-9 place-items-center rounded-lg text-muted transition-colors hover:bg-elevated hover:text-ink"
              >
                {meta.muted ? <BellSlash size={18} /> : <Bell size={18} />}
              </button>

              <button
                onClick={() => onCall(selected.id, selected.name, "audio")}
                aria-label="Voice call"
                className="grid h-9 w-9 place-items-center rounded-lg text-muted transition-colors hover:bg-elevated hover:text-ink"
              >
                <Phone size={18} />
              </button>
              <button
                onClick={() => onCall(selected.id, selected.name, "video")}
                aria-label="Video call"
                className="grid h-9 w-9 place-items-center rounded-lg text-muted transition-colors hover:bg-elevated hover:text-ink"
              >
                <VideoCamera size={18} />
              </button>
            </div>

            {ttlOpen && (
              <>
                <div className="fixed inset-0 z-20" onClick={() => setTtlOpen(false)} />
                <div className="absolute right-4 top-14 z-30 w-60 overflow-hidden rounded-xl bg-surface py-1 text-sm shadow-pop">
                  <p className="px-3 py-2 text-[12px] leading-snug text-muted">
                    New messages in this chat will disappear after the selected time — on both
                    devices.
                  </p>
                  <div className="h-px bg-line" />
                  {TTL_PRESETS.map((p) => (
                    <button
                      key={p.seconds}
                      onClick={() => {
                        onSetTtl(selected.id, p.seconds);
                        setTtlOpen(false);
                      }}
                      className={cn(
                        "flex w-full items-center justify-between px-3 py-2 text-left transition-colors hover:bg-elevated",
                        (meta.ttl ?? 0) === p.seconds && "text-brand",
                      )}
                    >
                      {p.label}
                      {(meta.ttl ?? 0) === p.seconds && <span className="text-brand">✓</span>}
                    </button>
                  ))}
                </div>
              </>
            )}
          </div>

          <Chat
            messages={messages}
            onSend={(payload) => onSend(selected.id, payload)}
            actions={actions}
            onEditSubmit={(mid, t) => onEditMessage(selected.id, mid, t)}
            onTyping={(on) => onTyping(selected.id, on)}
            peerTyping={!!typing[selected.id]}
            pinnedMid={meta.pinned}
            onUnpin={() => onPin(selected.id, null)}
            search={search}
            placeholder={`Message ${selected.name.split(" ")[0]}`}
          />
        </div>
      ) : (
        <div className="grid flex-1 place-items-center text-sm text-muted">
          Pick a conversation to start chatting.
        </div>
      )}

      {/* Forward picker */}
      {forwarding && selected && (
        <div
          className="fixed inset-0 z-50 grid place-items-center bg-black/50 p-6"
          onClick={() => setForwarding(null)}
        >
          <div
            onClick={(e) => e.stopPropagation()}
            className="w-full max-w-sm overflow-hidden rounded-xl border border-line bg-surface"
          >
            <div className="flex items-center gap-2 border-b border-line px-4 py-3">
              <ArrowBendUpRight size={17} className="text-brand" />
              <h2 className="text-sm font-semibold">Forward to</h2>
              <button
                onClick={() => setForwarding(null)}
                aria-label="Close"
                className="ml-auto text-muted hover:text-ink"
              >
                <X size={16} />
              </button>
            </div>
            <p className="truncate border-b border-line px-4 py-2 text-[12.5px] text-muted">
              {summarize(forwarding)}
            </p>
            <div className="max-h-72 overflow-y-auto p-2">
              {friends.map((f) => (
                <button
                  key={f.id}
                  onClick={() => {
                    onForward(f.id, forwarding, forwarding.mine ? "You" : selected.name);
                    setForwarding(null);
                    setSelectedId(f.id);
                  }}
                  className="flex w-full items-center gap-3 rounded-lg px-2.5 py-2 text-left transition-colors hover:bg-elevated"
                >
                  <Avatar name={f.name} size={32} />
                  <span className="truncate text-sm">{f.name}</span>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
