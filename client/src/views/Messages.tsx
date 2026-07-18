import { useMemo, useState } from "react";
import { Phone, VideoCamera } from "@phosphor-icons/react";
import { Avatar } from "@/components/Avatar";
import { Chat } from "@/components/Chat";
import { cn } from "@/lib/utils";
import { conversations, type ChatMessage, type Friend, type Presence } from "@/lib/mock";
import type { Call } from "@/components/CallOverlay";

const dot: Record<Presence, string> = {
  online: "bg-good",
  workspace: "bg-warn",
  offline: "bg-muted/50",
};

let nextMsgId = 5000;

export function Messages({
  friends,
  initialFriendId,
  onCall,
}: {
  friends: Friend[];
  initialFriendId: number | null;
  onCall: (call: Call) => void;
}) {
  const [threads, setThreads] = useState<Record<number, ChatMessage[]>>(() => ({
    ...conversations,
  }));
  const [selectedId, setSelectedId] = useState<number | null>(
    initialFriendId ?? friends[0]?.id ?? null,
  );

  const selected = friends.find((f) => f.id === selectedId) ?? null;
  const messages = selectedId ? (threads[selectedId] ?? []) : [];

  const preview = useMemo(() => {
    const p: Record<number, string> = {};
    for (const f of friends) {
      const t = threads[f.id];
      p[f.id] = t && t.length ? t[t.length - 1].text : "No messages yet";
    }
    return p;
  }, [friends, threads]);

  const send = (text: string) => {
    if (!selectedId) return;
    const now = new Date();
    const time = `${String(now.getHours()).padStart(2, "0")}:${String(
      now.getMinutes(),
    ).padStart(2, "0")}`;
    setThreads((prev) => ({
      ...prev,
      [selectedId]: [
        ...(prev[selectedId] ?? []),
        { id: nextMsgId++, mine: true, text, time },
      ],
    }));
  };

  return (
    <div className="animate-fade-up flex h-[calc(100vh-11rem)] overflow-hidden rounded-xl border border-line bg-surface">
      {/* Conversation list */}
      <div className="flex w-72 shrink-0 flex-col border-r border-line">
        <div className="border-b border-line px-4 py-3">
          <h1 className="text-base font-semibold">Messages</h1>
        </div>
        <div className="flex-1 overflow-y-auto p-2">
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
            onSend={send}
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
