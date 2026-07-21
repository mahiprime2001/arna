import { useEffect, useState } from "react";
import {
  UserPlus,
  ChatCircle,
  VideoCamera,
  Trash,
  Check,
  ClockCountdown,
  MagnifyingGlass,
} from "@phosphor-icons/react";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { Avatar } from "@/components/Avatar";
import { PageHeader } from "@/components/PageHeader";
import { cn } from "@/lib/utils";
import * as api from "@/lib/api";
import type { Call } from "@/components/CallOverlay";
import type {
  Friend,
  FriendRequest,
  Presence,
  SearchResult,
  SentRequest,
} from "@/lib/mock";

function SectionTitle({ label, count }: { label: string; count?: number }) {
  return (
    <div className="mb-3 flex items-center gap-2">
      <h2 className="text-sm font-semibold text-muted">{label}</h2>
      {count !== undefined && count > 0 && (
        <span className="grid h-5 min-w-[20px] place-items-center rounded-full bg-brand-soft px-1.5 text-[11px] font-semibold text-brand">
          {count}
        </span>
      )}
    </div>
  );
}

function PresenceTag({ presence }: { presence: Presence }) {
  const map: Record<Presence, { dot: string; text: string; label: string }> = {
    online: { dot: "bg-good", text: "text-good", label: "Online" },
    workspace: { dot: "bg-warn", text: "text-warn", label: "In a workspace" },
    offline: { dot: "bg-muted/50", text: "text-muted", label: "Offline" },
  };
  const s = map[presence];
  return (
    <span className="inline-flex items-center gap-1.5 text-[12px]">
      <span className={cn("h-2 w-2 rounded-full", s.dot)} />
      <span className={s.text}>{s.label}</span>
    </span>
  );
}

// Live people search with an inline add / accept.
function FindPeople({ onAdd }: { onAdd: (handle: string) => Promise<void> }) {
  const [q, setQ] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const query = q.trim();
    if (!query) {
      setResults([]);
      return;
    }
    const h = setTimeout(async () => {
      try {
        const d = await api.searchUsers(query);
        setResults(d.users);
      } catch {
        setResults([]);
      }
    }, 250);
    return () => clearTimeout(h);
  }, [q]);

  const rerun = async () => {
    try {
      const d = await api.searchUsers(q.trim());
      setResults(d.users);
    } catch {
      /* ignore */
    }
  };

  const add = async (handle: string) => {
    setMsg("");
    setBusy(true);
    try {
      await onAdd(handle);
      setMsg(`Sent to ${handle}.`);
      await rerun();
    } catch (e) {
      setMsg(e instanceof Error ? e.message : "Could not send the request.");
    } finally {
      setBusy(false);
    }
  };

  const raw = q.trim().startsWith("@") ? q.trim() : `@${q.trim()}`;

  return (
    <Card className="p-4">
      <div className="relative">
        <MagnifyingGlass
          size={16}
          className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted"
        />
        <Input
          autoFocus
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Find people by name or handle"
          className="pl-9"
        />
      </div>
      {msg && <p className="mt-2.5 text-[13px] text-muted">{msg}</p>}

      <div className="mt-2">
        {results.map((u) => (
          <div key={u.id} className="flex items-center gap-3 rounded-lg px-1 py-2">
            <Avatar name={u.name} size={36} />
            <div className="min-w-0">
              <p className="text-sm font-medium leading-tight">{u.name}</p>
              <p className="text-[12.5px] text-muted">{u.handle}</p>
            </div>
            <div className="ml-auto">
              {u.status === "none" && (
                <Button size="sm" disabled={busy} onClick={() => add(u.handle)}>
                  <UserPlus size={15} weight="bold" /> Add
                </Button>
              )}
              {u.status === "incoming" && (
                <Button size="sm" disabled={busy} onClick={() => add(u.handle)}>
                  <Check size={15} weight="bold" /> Accept
                </Button>
              )}
              {u.status === "outgoing" && (
                <span className="text-[13px] text-muted">Requested</span>
              )}
              {u.status === "friends" && (
                <span className="inline-flex items-center gap-1 text-[13px] text-good">
                  <Check size={14} weight="bold" /> Friends
                </span>
              )}
            </div>
          </div>
        ))}

        {q.trim() && results.length === 0 && (
          <p className="px-1 py-2 text-[13px] text-muted">
            No one found.{" "}
            <button
              disabled={busy}
              onClick={() => add(raw)}
              className="font-medium text-brand hover:underline"
            >
              Send a request to {raw}
            </button>
          </p>
        )}
      </div>
    </Card>
  );
}

export function Friends({
  friends,
  requests,
  sent,
  onAccept,
  onDecline,
  onCancelSent,
  onRemove,
  onAdd,
  onMessage,
  onCall,
}: {
  friends: Friend[];
  requests: FriendRequest[];
  sent: SentRequest[];
  onAccept: (id: number) => void;
  onDecline: (id: number) => void;
  onCancelSent: (id: number) => void;
  onRemove: (userId: number) => void;
  onAdd: (handle: string) => Promise<void>;
  onMessage: (id: number) => void;
  onCall: (call: Call) => void;
}) {
  const [adding, setAdding] = useState(false);
  const [confirmId, setConfirmId] = useState<number | null>(null);

  return (
    <div className="animate-fade-up space-y-8">
      <PageHeader
        title="Friends"
        subtitle="People you can start workspaces with."
        action={
          <Button
            variant={adding ? "outline" : "primary"}
            onClick={() => setAdding((v) => !v)}
          >
            <UserPlus size={16} weight="bold" /> Add friend
          </Button>
        }
      />

      {adding && <FindPeople onAdd={onAdd} />}

      {requests.length > 0 && (
        <section>
          <SectionTitle label="Requests" count={requests.length} />
          <div className="space-y-2">
            {requests.map((r) => (
              <Card key={r.id} className="flex items-center gap-3 p-3.5">
                <Avatar name={r.name} size={40} />
                <div className="min-w-0">
                  <p className="font-medium leading-tight">{r.name}</p>
                  <p className="text-[13px] text-muted">{r.handle}</p>
                </div>
                <div className="ml-auto flex gap-2">
                  <Button size="sm" onClick={() => onAccept(r.id)}>
                    <Check size={15} weight="bold" /> Accept
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => onDecline(r.id)}>
                    Decline
                  </Button>
                </div>
              </Card>
            ))}
          </div>
        </section>
      )}

      <section>
        <SectionTitle label={`All friends · ${friends.length}`} />
        {friends.length === 0 ? (
          <Card className="flex flex-col items-center gap-2 px-6 py-14 text-center">
            <div className="grid h-12 w-12 place-items-center rounded-2xl bg-brand-soft">
              <UserPlus size={22} weight="duotone" className="text-brand" />
            </div>
            <p className="font-medium">No friends yet</p>
            <p className="max-w-xs text-sm text-muted">
              Use Add friend to find someone by their handle. When they accept,
              they show up here.
            </p>
          </Card>
        ) : (
          <div className="space-y-2">
            {friends.map((f) => (
              <Card
                key={f.id}
                className="group flex items-center gap-3 p-3.5 transition-colors hover:bg-elevated"
              >
                <Avatar name={f.name} size={40} />
                <div className="min-w-0">
                  <p className="font-medium leading-tight">{f.name}</p>
                  <p className="text-[13px] text-muted">{f.handle}</p>
                </div>
                <div className="ml-4 hidden sm:block">
                  <PresenceTag presence={f.presence} />
                </div>
                <div className="ml-auto flex items-center gap-2">
                  {confirmId === f.id ? (
                    <>
                      <span className="text-[13px] text-muted">Remove?</span>
                      <Button
                        size="sm"
                        variant="danger"
                        onClick={() => {
                          onRemove(f.id);
                          setConfirmId(null);
                        }}
                      >
                        Remove
                      </Button>
                      <Button size="sm" variant="ghost" onClick={() => setConfirmId(null)}>
                        Cancel
                      </Button>
                    </>
                  ) : (
                    <>
                      <Button size="sm" variant="outline" onClick={() => onMessage(f.id)}>
                        <ChatCircle size={15} /> Message
                      </Button>
                      {f.presence !== "offline" && (
                        <button
                          onClick={() => onCall({ name: f.name, kind: "video" })}
                          title="Video call"
                          className="grid h-8 w-8 place-items-center rounded-md text-muted transition-colors hover:bg-elevated hover:text-ink"
                        >
                          <VideoCamera size={16} />
                        </button>
                      )}
                      <button
                        onClick={() => setConfirmId(f.id)}
                        title="Remove friend"
                        className="grid h-8 w-8 place-items-center rounded-md text-muted opacity-0 transition-all hover:bg-danger/10 hover:text-danger group-hover:opacity-100"
                      >
                        <Trash size={16} />
                      </button>
                    </>
                  )}
                </div>
              </Card>
            ))}
          </div>
        )}
      </section>

      {sent.length > 0 && (
        <section>
          <SectionTitle label="Pending" />
          <div className="space-y-2">
            {sent.map((s) => (
              <Card key={s.id} className="flex items-center gap-3 p-3.5">
                <div className="grid h-10 w-10 place-items-center rounded-full bg-elevated text-muted">
                  <ClockCountdown size={20} />
                </div>
                <div>
                  <p className="font-medium leading-tight">{s.handle}</p>
                  <p className="text-[13px] text-muted">Request sent</p>
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  className="ml-auto"
                  onClick={() => onCancelSent(s.id)}
                >
                  Cancel
                </Button>
              </Card>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
