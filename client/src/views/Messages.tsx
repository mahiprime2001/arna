import { useEffect, useMemo, useState } from "react";
import { Phone, VideoCamera } from "@phosphor-icons/react";
import { Avatar } from "@/components/Avatar";
import { Chat } from "@/components/Chat";
import { cn } from "@/lib/utils";
import type { ChatMessage, Friend, OutgoingPayload, Presence } from "@/lib/mock";
import type { Threads } from "@/lib/chat";
import type { Call } from "@/components/CallOverlay";

function previewOf(msgs?: ChatMessage[]): string {
  if (!msgs || !msgs.length) return "No messages yet";
  const m = msgs[msgs.length - 1];
  if (m.kind === "image") return "Photo";
  if (m.kind === "audio") return "Voice message";
  if (m.kind === "file") return m.media?.name || "File";
  return m.text || "";
}

const dot: Record<Presence, string> = {
  online: "bg-good",
  workspace: "bg-warn",
  offline: "bg-muted/50",
};

export function Messages({
  friends,
  chats,
  unread,
  initialFriendId,
  onOpen,
  onSend,
  onCall,
}: {
  friends: Friend[];
  chats: Threads;
  unread: Record<number, number>;
  initialFriendId: number | null;
  onOpen: (friendId: number) => void;
  onSend: (friendId: number, payload: OutgoingPayload) => void;
  onCall: (call: Call) => void;
}) {
  const [selectedId, setSelectedId] = useState<number | null>(
    initialFriendId ?? friends[0]?.id ?? null,
  );

  // Opening a conversation clears its unread badge (in the parent).
  useEffect(() => {
    if (selectedId != null) onOpen(selectedId);
  }, [selectedId, onOpen]);

  const selected = friends.find((f) => f.id === selectedId) ?? null;
  const messages = selectedId != null ? (chats[selectedId] ?? []) : [];

  const preview = useMemo(() => {
    const p: Record<number, string> = {};
    for (const f of friends) p[f.id] = previewOf(chats[f.id]);
    return p;
  }, [friends, chats]);

  return (
    <div className="animate-fade-up flex h-[calc(100vh-11rem)] overflow-hidden rounded-xl border border-line bg-surface">
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
          {friends.map((f) => (
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
                <p className="truncate text-sm font-medium">{f.name}</p>
                <p className="truncate text-[12.5px] text-muted">{preview[f.id]}</p>
              </div>
              {unread[f.id] > 0 && (
                <span className="grid h-5 min-w-[20px] place-items-center rounded-full bg-brand px-1.5 text-[11px] font-semibold text-brand-fg">
                  {unread[f.id]}
                </span>
              )}
            </button>
          ))}
        </div>
      </div>

      {/* Thread */}
      {selected ? (
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex items-center gap-3 border-b border-line px-4 py-2.5">
            <Avatar name={selected.name} size={34} />
            <div className="min-w-0">
              <p className="text-sm font-medium leading-tight">{selected.name}</p>
              <p className="text-[12px] capitalize text-muted">
                {selected.presence === "workspace" ? "In a workspace" : selected.presence}
              </p>
            </div>
            <div className="ml-auto flex items-center gap-1">
              <button
                onClick={() => onCall({ name: selected.name, kind: "audio" })}
                aria-label="Voice call"
                className="grid h-9 w-9 place-items-center rounded-lg text-muted transition-colors hover:bg-elevated hover:text-ink"
              >
                <Phone size={18} />
              </button>
              <button
                onClick={() => onCall({ name: selected.name, kind: "video" })}
                aria-label="Video call"
                className="grid h-9 w-9 place-items-center rounded-lg text-muted transition-colors hover:bg-elevated hover:text-ink"
              >
                <VideoCamera size={18} />
              </button>
            </div>
          </div>
          <Chat
            messages={messages}
            onSend={(payload) => onSend(selected.id, payload)}
            placeholder={`Message ${selected.name.split(" ")[0]}`}
          />
        </div>
      ) : (
        <div className="grid flex-1 place-items-center text-sm text-muted">
          Pick a conversation to start chatting.
        </div>
      )}
    </div>
  );
}
