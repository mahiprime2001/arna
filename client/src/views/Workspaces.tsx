import { useState } from "react";
import {
  Plus,
  StackPlus,
  Play,
  Pause,
  FloppyDisk,
  Trash,
  Copy,
  Globe,
  GlobeX,
  Clock,
  Monitor,
  Cube,
  Cloud,
  SignIn,
  FolderOpen,
} from "@phosphor-icons/react";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { PageHeader } from "@/components/PageHeader";
import { CreateWorkspace } from "@/components/CreateWorkspace";
import { OverlayReview } from "@/components/OverlayReview";
import { isTauri } from "@/lib/wse";
import { cn } from "@/lib/utils";
import {
  canTransition,
  describe,
  STATE_LABEL,
  type GuaranteeStrength,
  type Workspace,
  type WorkspaceState,
} from "@/lib/workspace";

// Honest guarantees, rendered straight from the runtime — green when it's a real
// wall (strong/isolated), amber when it's shared/partial, muted when unsupported.
const guarTone: Record<GuaranteeStrength, string> = {
  strong: "text-good",
  isolated: "text-good",
  shared: "text-warn",
  partial: "text-warn",
  unsupported: "text-muted",
};

function Guarantee({ label, value }: { label: string; value: GuaranteeStrength }) {
  return (
    <span className="flex items-center justify-between gap-2">
      <span className="text-muted">{label}</span>
      <span className={cn("font-medium capitalize", guarTone[value])}>{value}</span>
    </span>
  );
}

const stateTone: Record<WorkspaceState, string> = {
  created: "bg-muted/20 text-muted",
  running: "bg-good/15 text-good",
  idle: "bg-warn/15 text-warn",
  paused: "bg-warn/15 text-warn",
  resuming: "bg-brand/15 text-brand-strong",
  saved: "bg-brand/15 text-brand-strong",
  archived: "bg-muted/20 text-muted",
  deleted: "bg-danger/15 text-danger",
};

function WorkspaceCard({
  w,
  onTransition,
  onDelete,
  onOpen,
  onReview,
}: {
  w: Workspace;
  onTransition: (id: string, to: WorkspaceState) => void;
  onDelete: (w: Workspace) => void;
  onOpen: (w: Workspace) => void;
  onReview: (w: Workspace) => void;
}) {
  const [copied, setCopied] = useState(false);
  const copyId = () => {
    navigator.clipboard?.writeText(w.id);
    setCopied(true);
    setTimeout(() => setCopied(false), 1600);
  };
  const [copiedUrl, setCopiedUrl] = useState(false);
  const copyLan = () => {
    if (!w.lanUrl) return;
    navigator.clipboard?.writeText(w.lanUrl);
    setCopiedUrl(true);
    setTimeout(() => setCopiedUrl(false), 1600);
  };

  // Buttons are offered only for transitions the spec permits from here.
  const can = (to: WorkspaceState) => canTransition(w.state, to);

  return (
    <Card className="p-5">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h3 className="truncate text-[15px] font-semibold">{w.name}</h3>
            <span
              className={cn(
                "shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium",
                stateTone[w.state],
              )}
            >
              {STATE_LABEL[w.state]}
            </span>
            <span className="ml-auto inline-flex shrink-0 items-center gap-1 rounded-full bg-elevated px-2 py-0.5 text-[11px] text-muted">
              {w.runtime === "docker" ? (
                <>
                  <Cube size={12} weight="fill" /> Docker
                </>
              ) : w.runtime === "cloud" ? (
                <>
                  <Cloud size={12} weight="fill" /> Cloud
                </>
              ) : (
                <>
                  <Monitor size={12} weight="fill" /> Native
                </>
              )}
            </span>
          </div>
          <p className="mt-1 text-[12.5px] text-muted">{describe(w)}</p>

          <div className="mt-2.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11.5px] text-muted">
            <span className="inline-flex items-center gap-1">
              {w.internet ? <Globe size={13} /> : <GlobeX size={13} />}
              {w.internet ? "Internet" : "Offline"}
            </span>
            {w.persistence === "temporary" && (
              <span className="inline-flex items-center gap-1">
                <Clock size={13} /> Wipes on close
              </span>
            )}
            {(w.limits.cpuCores || w.limits.memoryGb) && (
              <span>
                {[
                  w.limits.cpuCores && `${w.limits.cpuCores} cores`,
                  w.limits.memoryGb && `${w.limits.memoryGb} GB`,
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
            )}
          </div>

          {w.guarantees && (
            <div className="mt-3 grid grid-cols-2 gap-x-4 gap-y-1 rounded-md border border-line bg-elevated/40 px-2.5 py-2 text-[11px]">
              <Guarantee label="Files" value={w.guarantees.overlay} />
              <Guarantee label="Network" value={w.guarantees.network} />
              <Guarantee label="Registry" value={w.guarantees.registry} />
              <Guarantee label="Clipboard" value={w.guarantees.clipboard} />
            </div>
          )}

          <button
            onClick={copyId}
            title="Copy the invite ID"
            className="mt-3 inline-flex items-center gap-1.5 rounded-md bg-elevated px-2 py-1 font-mono text-[11px] text-muted transition-colors hover:text-ink"
          >
            <Copy size={12} />
            {copied ? "Copied" : w.id}
          </button>

          {w.lanUrl && (
            <div className="mt-3 rounded-md border border-line bg-elevated/50 p-2.5">
              <p className="text-[11px] font-medium text-muted">
                Invite link — friends on your Wi-Fi can join
              </p>
              <button
                onClick={copyLan}
                title="Copy the invite link"
                className="mt-1 inline-flex max-w-full items-center gap-1.5 font-mono text-[11.5px] text-ink transition-colors hover:text-muted"
              >
                <Copy size={12} className="shrink-0" />
                <span className="truncate">{copiedUrl ? "Copied — send this to a friend" : w.lanUrl}</span>
              </button>
            </div>
          )}
        </div>
      </div>

      <div className="mt-4 flex flex-wrap gap-2 border-t border-line pt-3.5">
        {(w.state === "running" || w.state === "idle") && (
          <Button size="sm" onClick={() => onOpen(w)}>
            <Monitor size={14} weight="fill" /> Open
          </Button>
        )}
        {w.state === "saved" && (
          <Button size="sm" onClick={() => onOpen(w)}>
            <Play size={14} weight="fill" /> Resume
          </Button>
        )}
        {isTauri() && (w.runtime === "native" || !w.runtime) && (
          <Button size="sm" variant="outline" onClick={() => onReview(w)}>
            <FolderOpen size={14} /> Files
          </Button>
        )}
        {can("running") && (
          <Button size="sm" variant={w.state === "created" ? "primary" : "outline"} onClick={() => onTransition(w.id, "running")}>
            <Play size={14} weight="fill" /> {w.state === "created" ? "Start" : "Resume"}
          </Button>
        )}
        {can("resuming") && (
          <Button size="sm" onClick={() => onTransition(w.id, "resuming")}>
            <Play size={14} weight="fill" /> Resume
          </Button>
        )}
        {can("paused") && (
          <Button size="sm" variant="outline" onClick={() => onTransition(w.id, "paused")}>
            <Pause size={14} weight="fill" /> Pause
          </Button>
        )}
        {can("saved") && w.persistence === "saved" && (
          <Button size="sm" variant="outline" onClick={() => onTransition(w.id, "saved")}>
            <FloppyDisk size={14} /> Save and close
          </Button>
        )}
        <Button size="sm" variant="danger" className="ml-auto" onClick={() => onDelete(w)}>
          <Trash size={14} /> Delete
        </Button>
      </div>
    </Card>
  );
}

export function Workspaces({
  workspaces,
  onCreate,
  onTransition,
  onDelete,
  onOpen,
  onJoin,
}: {
  workspaces: Workspace[];
  onCreate: (draft: Omit<Workspace, "id" | "state" | "createdAt">) => void;
  onTransition: (id: string, to: WorkspaceState) => void;
  onDelete: (w: Workspace) => void;
  onOpen: (w: Workspace) => void;
  onJoin?: (link: string) => string | null;
}) {
  const [creating, setCreating] = useState(false);
  const [joining, setJoining] = useState(false);
  const [joinLink, setJoinLink] = useState("");
  const [joinError, setJoinError] = useState<string | null>(null);
  const [reviewing, setReviewing] = useState<Workspace | null>(null);

  const submitJoin = () => {
    const err = onJoin?.(joinLink) ?? "Joining isn't available here.";
    if (err) {
      setJoinError(err);
    } else {
      setJoinLink("");
      setJoining(false);
    }
  };

  return (
    <div className="animate-fade-up space-y-6">
      <PageHeader
        title="Workspaces"
        subtitle="Isolated places you lend to people you invite."
        action={
          <div className="flex gap-2">
            <Button variant="outline" onClick={() => setJoining((v) => !v)}>
              <SignIn size={16} weight="bold" /> Join
            </Button>
            <Button onClick={() => setCreating(true)}>
              <Plus size={16} weight="bold" /> New workspace
            </Button>
          </div>
        }
      />

      {joining && (
        <Card className="p-4">
          <h3 className="text-sm font-semibold">Join a friend's workspace</h3>
          <p className="mt-1 text-[12.5px] text-muted">
            Paste the invite link they sent you. You both need to be on the same Wi-Fi.
          </p>
          <div className="mt-3 flex flex-col gap-2 sm:flex-row">
            <input
              value={joinLink}
              onChange={(e) => {
                setJoinLink(e.target.value);
                setJoinError(null);
              }}
              onKeyDown={(e) => e.key === "Enter" && submitJoin()}
              placeholder="http://192.168.1.5:49153"
              className="flex-1 rounded-md border border-line bg-elevated px-3 py-2 font-mono text-[12.5px] outline-none focus:border-brand"
            />
            <Button onClick={submitJoin}>
              <SignIn size={14} weight="fill" /> Join
            </Button>
          </div>
          {joinError && <p className="mt-2 text-[12px] text-danger">{joinError}</p>}
        </Card>
      )}

      {workspaces.length === 0 ? (
        <Card className="flex flex-col items-center gap-3 px-6 py-16 text-center">
          <div className="grid h-14 w-14 place-items-center rounded-2xl bg-brand-soft">
            <StackPlus size={26} weight="duotone" className="text-brand" />
          </div>
          <div className="max-w-sm">
            <h3 className="text-base font-semibold">No workspaces yet</h3>
            <p className="mt-1 text-sm text-muted">
              Create one to lend compute to a friend. They get their own screen, and you
              keep working.
            </p>
          </div>
          <Button className="mt-1" onClick={() => setCreating(true)}>
            <Plus size={16} weight="bold" /> Create workspace
          </Button>
        </Card>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2">
          {workspaces.map((w) => (
            <WorkspaceCard
              key={w.id}
              w={w}
              onTransition={onTransition}
              onDelete={onDelete}
              onOpen={onOpen}
              onReview={setReviewing}
            />
          ))}
        </div>
      )}

      {creating && (
        <CreateWorkspace
          onClose={() => setCreating(false)}
          onCreate={(draft) => {
            onCreate(draft);
            setCreating(false);
          }}
        />
      )}

      {reviewing && (
        <OverlayReview workspace={reviewing} onClose={() => setReviewing(null)} />
      )}
    </div>
  );
}
