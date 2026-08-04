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
} from "@phosphor-icons/react";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { PageHeader } from "@/components/PageHeader";
import { CreateWorkspace } from "@/components/CreateWorkspace";
import { cn } from "@/lib/utils";
import {
  canTransition,
  describe,
  STATE_LABEL,
  type Workspace,
  type WorkspaceState,
} from "@/lib/workspace";

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
}: {
  w: Workspace;
  onTransition: (id: string, to: WorkspaceState) => void;
  onDelete: (w: Workspace) => void;
  onOpen: (w: Workspace) => void;
}) {
  const [copied, setCopied] = useState(false);
  const copyId = () => {
    navigator.clipboard?.writeText(w.id);
    setCopied(true);
    setTimeout(() => setCopied(false), 1600);
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

          <button
            onClick={copyId}
            title="Copy the invite ID"
            className="mt-3 inline-flex items-center gap-1.5 rounded-md bg-elevated px-2 py-1 font-mono text-[11px] text-muted transition-colors hover:text-ink"
          >
            <Copy size={12} />
            {copied ? "Copied" : w.id}
          </button>
        </div>
      </div>

      <div className="mt-4 flex flex-wrap gap-2 border-t border-line pt-3.5">
        {(w.state === "running" || w.state === "idle") && (
          <Button size="sm" onClick={() => onOpen(w)}>
            <Monitor size={14} weight="fill" /> Open
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
}: {
  workspaces: Workspace[];
  onCreate: (draft: Omit<Workspace, "id" | "state" | "createdAt">) => void;
  onTransition: (id: string, to: WorkspaceState) => void;
  onDelete: (w: Workspace) => void;
  onOpen: (w: Workspace) => void;
}) {
  const [creating, setCreating] = useState(false);

  return (
    <div className="animate-fade-up space-y-6">
      <PageHeader
        title="Workspaces"
        subtitle="Isolated places you lend to people you invite."
        action={
          <Button onClick={() => setCreating(true)}>
            <Plus size={16} weight="bold" /> New workspace
          </Button>
        }
      />

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
    </div>
  );
}
