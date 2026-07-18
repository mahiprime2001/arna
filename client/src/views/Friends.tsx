import { useState } from "react";
import {
  UserPlus,
  PaperPlaneTilt,
  Trash,
  Check,
  X,
  ClockCountdown,
} from "@phosphor-icons/react";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { Avatar } from "@/components/Avatar";
import { PageHeader } from "@/components/PageHeader";
import { cn } from "@/lib/utils";
import type { Friend, FriendRequest, Presence, SentRequest } from "@/lib/mock";

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

export function Friends({
  friends,
  requests,
  sent,
  onAccept,
  onDecline,
  onCancelSent,
  onRemove,
  onAdd,
}: {
  friends: Friend[];
  requests: FriendRequest[];
  sent: SentRequest[];
  onAccept: (id: number) => void;
  onDecline: (id: number) => void;
  onCancelSent: (id: number) => void;
  onRemove: (id: number) => void;
  onAdd: (handle: string) => void;
}) {
  const [adding, setAdding] = useState(false);
  const [handle, setHandle] = useState("");
  const [confirmId, setConfirmId] = useState<number | null>(null);

  const submitAdd = (e: React.FormEvent) => {
    e.preventDefault();
    const h = handle.trim();
    if (!h) return;
    onAdd(h.startsWith("@") ? h : `@${h}`);
    setHandle("");
    setAdding(false);
  };

  return (
    <div className="animate-fade-up space-y-8">
      <PageHeader
        title="Friends"
        subtitle="People you can start workspaces with."
        action={
          <Button variant={adding ? "outline" : "primary"} onClick={() => setAdding((v) => !v)}>
            <UserPlus size={16} weight="bold" /> Add friend
          </Button>
        }
      />

      {adding && (
        <Card className="p-4">
          <form onSubmit={submitAdd} className="flex gap-2">
            <Input
              autoFocus
              value={handle}
              onChange={(e) => setHandle(e.target.value)}
              placeholder="Their handle or email, like @sam"
            />
            <Button type="submit" disabled={!handle.trim()}>
              Send request
            </Button>
          </form>
          <p className="mt-2.5 text-[13px] text-muted">
            They will get a request to connect. Once they accept, you can start a
            workspace together.
          </p>
        </Card>
      )}

      {requests.length > 0 && (
        <section>
          <SectionTitle label="Requests" count={requests.length} />
          <div className="space-y-2">
            {requests.map((r) => (
              <Card key={r.id} className="flex items-center gap-3 p-3.5">
                <Avatar name={r.name} size={40} />
                <div className="min-w-0">
                  <p className="font-medium leading-tight">{r.name}</p>
                  <p className="text-[13px] text-muted">
                    {r.handle} · {r.mutual} mutual
                  </p>
                </div>
                <div className="ml-auto flex gap-2">
                  <Button size="sm" onClick={() => onAccept(r.id)}>
                    <Check size={15} weight="bold" /> Accept
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => onDecline(r.id)}>
                    <X size={15} weight="bold" /> Decline
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
              Add someone by their handle. When they accept, they show up here.
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
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={f.presence === "offline"}
                        title={
                          f.presence === "offline"
                            ? "They are offline right now"
                            : "Invite to a workspace"
                        }
                      >
                        <PaperPlaneTilt size={15} /> Invite
                      </Button>
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
