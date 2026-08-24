// Invite someone to a workspace at a ROLE. The generated token identifies an
// invitation + a role + how to reach the workspace — it is never raw authority.
// v1 carries the role (Viewer default); the workspace decides enforcement, which
// deepens with Watch & Control.
import { useState } from "react";
import { createPortal } from "react-dom";
import { X, Copy } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { makeInvite } from "@/lib/wse";
import { ROLE_HINT, ROLE_LABEL, type Workspace, type WorkspaceRole } from "@/lib/workspace";

const ROLES: WorkspaceRole[] = ["viewer", "collaborator", "controller"];

export function InviteModal({
  workspace,
  hostId,
  onClose,
}: {
  workspace: Workspace;
  hostId: number;
  onClose: () => void;
}) {
  const [role, setRole] = useState<WorkspaceRole>("viewer");
  const [copied, setCopied] = useState(false);

  const token = makeInvite(hostId, workspace.id, workspace.name, role);

  const copy = () => {
    navigator.clipboard?.writeText(token);
    setCopied(true);
    setTimeout(() => setCopied(false), 1600);
  };

  return createPortal(
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-xl border border-line bg-canvas shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between border-b border-line px-5 py-3.5">
          <div>
            <h2 className="text-[15px] font-semibold">Invite to {workspace.name}</h2>
            <p className="mt-0.5 text-[12px] text-muted">
              The invite grants a role. The workspace decides what that role may do — the
              link is never full control.
            </p>
          </div>
          <button onClick={onClose} className="shrink-0 text-muted transition-colors hover:text-ink">
            <X size={18} />
          </button>
        </div>

        <div className="px-5 py-4">
          <p className="mb-2 text-[12px] font-medium text-muted">Role</p>
          <div className="grid gap-2">
            {ROLES.map((r) => (
              <button
                key={r}
                onClick={() => setRole(r)}
                className={cn(
                  "flex items-center justify-between rounded-md border px-3 py-2 text-left transition-colors",
                  role === r ? "border-brand bg-brand-soft" : "border-line hover:border-muted",
                )}
              >
                <span className="text-[13px] font-medium">{ROLE_LABEL[r]}</span>
                <span className="text-[11.5px] text-muted">{ROLE_HINT[r]}</span>
              </button>
            ))}
          </div>

          <p className="mb-2 mt-4 text-[12px] font-medium text-muted">Invite code</p>
          <button
            onClick={copy}
            title="Copy the invite code"
            className="flex w-full items-center gap-2 rounded-md border border-line bg-elevated px-3 py-2 text-left font-mono text-[11.5px] transition-colors hover:text-muted"
          >
            <Copy size={13} className="shrink-0" />
            <span className="truncate">{copied ? "Copied — send this to a friend" : token}</span>
          </button>
          <p className="mt-3 text-[11.5px] text-muted">
            Your friend pastes this into <span className="font-medium text-ink">Join</span>. It
            sends you a request to approve — the code itself grants nothing.
          </p>
        </div>
      </div>
    </div>,
    document.body,
  );
}
