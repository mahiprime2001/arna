// Host-side approval for a request-to-join. The invite only IDENTIFIES the
// workspace + role; this approval is what actually creates access (a Watch &
// Control session). Decline sends nothing back but a decline; approve starts the
// session.
import { createPortal } from "react-dom";
import { UserPlus } from "@phosphor-icons/react";
import { Button } from "@/components/ui/Button";
import { ROLE_LABEL, type WorkspaceRole } from "@/lib/workspace";

export function JoinRequestPrompt({
  guestName,
  workspaceName,
  role,
  onApprove,
  onDecline,
}: {
  guestName: string;
  workspaceName: string;
  role: WorkspaceRole;
  onApprove: () => void;
  onDecline: () => void;
}) {
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
              <span className="font-medium text-ink">{workspaceName}</span> as{" "}
              {ROLE_LABEL[role]}.
            </p>
          </div>
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={onDecline}>
            Decline
          </Button>
          <Button size="sm" onClick={onApprove}>
            Approve
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
