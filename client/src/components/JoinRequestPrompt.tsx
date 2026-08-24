// Host-side approval for a request-to-join, with a capability-driven connection
// chooser. The invite only IDENTIFIES the workspace + role; this approval is what
// creates access. The two modes are enabled by what the runtime can actually
// provide — the UI itself is runtime-agnostic (no Docker/Native branching here).
import { useState } from "react";
import { createPortal } from "react-dom";
import { UserPlus, DoorOpen, Broadcast } from "@phosphor-icons/react";
import { Button } from "@/components/ui/Button";
import { cn } from "@/lib/utils";
import { ROLE_LABEL, type WorkspaceRole } from "@/lib/workspace";

export type ConnectionMode = "enter" | "watch";

function ModeOption({
  active,
  enabled,
  onClick,
  icon,
  title,
  desc,
}: {
  active: boolean;
  enabled: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  title: string;
  desc: string;
}) {
  return (
    <button
      onClick={enabled ? onClick : undefined}
      disabled={!enabled}
      className={cn(
        "flex w-full items-start gap-3 rounded-lg border p-3 text-left transition-colors",
        !enabled
          ? "cursor-not-allowed border-line opacity-50"
          : active
            ? "border-brand bg-brand-soft"
            : "border-line hover:bg-elevated",
      )}
    >
      <span
        className={cn(
          "mt-0.5 grid h-4 w-4 shrink-0 place-items-center rounded-full border",
          active && enabled ? "border-brand" : "border-muted",
        )}
      >
        {active && enabled && <span className="h-2 w-2 rounded-full bg-brand" />}
      </span>
      <span className="min-w-0">
        <span className="flex items-center gap-1.5 text-[13px] font-medium">
          {icon} {title}
        </span>
        <span className="mt-0.5 block text-[12px] text-muted">{desc}</span>
      </span>
    </button>
  );
}

export function JoinRequestPrompt({
  guestName,
  workspaceName,
  role,
  canEnter,
  canWatch,
  onApprove,
  onDecline,
}: {
  guestName: string;
  workspaceName: string;
  role: WorkspaceRole;
  canEnter: boolean;
  canWatch: boolean;
  onApprove: (mode: ConnectionMode) => void;
  onDecline: () => void;
}) {
  const [mode, setMode] = useState<ConnectionMode>(canEnter ? "enter" : "watch");

  return createPortal(
    <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/50 p-4">
      <div className="w-full max-w-sm rounded-xl border border-line bg-canvas p-5 shadow-2xl">
        <div className="flex items-start gap-3">
          <div className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-brand-soft text-brand">
            <UserPlus size={18} weight="fill" />
          </div>
          <div className="min-w-0">
            <h2 className="text-[15px] font-semibold">Join request</h2>
            <p className="mt-1 text-[13px] leading-relaxed text-muted">
              <span className="font-medium text-ink">{guestName}</span> wants to join{" "}
              <span className="font-medium text-ink">{workspaceName}</span> as {ROLE_LABEL[role]}.
            </p>
          </div>
        </div>

        <p className="mb-2 mt-4 text-[12px] font-medium text-muted">How should they connect?</p>
        <div className="space-y-2">
          <ModeOption
            active={mode === "enter"}
            enabled={canEnter}
            onClick={() => setMode("enter")}
            icon={<DoorOpen size={14} />}
            title="Enter workspace"
            desc={canEnter ? "Direct workspace access" : "Native surface not ready yet"}
          />
          <ModeOption
            active={mode === "watch"}
            enabled={canWatch}
            onClick={() => setMode("watch")}
            icon={<Broadcast size={14} />}
            title="Watch & Control"
            desc={canWatch ? "Stream a view + controlled input" : "Not available"}
          />
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={onDecline}>
            Decline
          </Button>
          <Button size="sm" onClick={() => onApprove(mode)} disabled={!canEnter && !canWatch}>
            Approve
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
