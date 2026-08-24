// Host side of Watch & Control: pick a friend, start sharing the workspace
// surface, then Grant / Revoke / Disconnect. Every button calls the backend gate
// — the UI never grants control by itself.
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/Button";
import { DockerSurface, sessionHub, type SurfaceProvider } from "@/lib/workspace-session";
import type { Workspace } from "@/lib/workspace";

type Friend = { id: number; name: string };

export function ShareSession({
  workspace,
  friends = [],
  initialGuest,
  surface,
  onClose,
}: {
  workspace: Workspace;
  friends?: Friend[];
  /** When set (approved a join request), share straight to this guest — no picker. */
  initialGuest?: Friend;
  /** Which surface to capture. Defaults to the window picker (Docker/Watch); a
   *  native workspace surface auto-captures the workspace instead. */
  surface?: SurfaceProvider;
  onClose: () => void;
}) {
  const [sharing, setSharing] = useState(false);
  const [role, setRole] = useState("viewer");
  const [error, setError] = useState<string | null>(null);

  // Reflect the guest's real role from the gate — the source of truth.
  useEffect(() => {
    if (!sharing) return;
    const t = setInterval(async () => {
      try {
        const s = JSON.parse(await sessionHub.sessionState());
        const g = (s.guests ?? [])[0];
        setRole(g ? g.role : "viewer");
      } catch {
        /* ignore */
      }
    }, 1000);
    return () => clearInterval(t);
  }, [sharing]);

  const start = async (f: Friend) => {
    setError(null);
    try {
      await sessionHub.startShare(workspace.id, f.id, String(f.id), f.name, surface ?? DockerSurface);
      setSharing(true);
    } catch {
      setError("Couldn't start sharing — pick the workspace window when prompted, and try again.");
    }
  };

  const disconnect = () => {
    sessionHub.disconnectHost();
    onClose();
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
            <h2 className="text-[15px] font-semibold">Share {workspace.name}</h2>
            <p className="mt-0.5 text-[12px] text-muted">
              They see the workspace; you decide if they can control it.
            </p>
          </div>
          <button onClick={onClose} className="shrink-0 text-muted transition-colors hover:text-ink">
            <X size={18} />
          </button>
        </div>

        <div className="px-5 py-4">
          {!sharing ? (
            <>
              {initialGuest ? (
                <>
                  <p className="mb-3 text-[13px] text-muted">
                    Share <span className="font-medium text-ink">{workspace.name}</span> with{" "}
                    <span className="font-medium text-ink">{initialGuest.name}</span>.{" "}
                    {surface ? "The workspace is captured directly." : "You'll pick which window to show."}
                  </p>
                  <Button className="w-full" onClick={() => start(initialGuest)}>
                    Start sharing
                  </Button>
                </>
              ) : (
                <>
                  <p className="mb-2 text-[12px] font-medium text-muted">Share with</p>
                  {friends.length === 0 ? (
                    <p className="text-[13px] text-muted">
                      Add a friend first to share a workspace with them.
                    </p>
                  ) : (
                    <div className="grid gap-2">
                      {friends.map((f) => (
                        <Button key={f.id} variant="outline" onClick={() => start(f)}>
                          {f.name}
                        </Button>
                      ))}
                    </div>
                  )}
                </>
              )}
              {error && <p className="mt-3 text-[12px] text-danger">{error}</p>}
            </>
          ) : (
            <>
              <div className="flex items-center justify-between rounded-md border border-line bg-elevated px-3 py-2">
                <span className="text-[13px]">Guest</span>
                <span className="text-[12px] font-medium capitalize">{role}</span>
              </div>
              <div className="mt-3 grid grid-cols-2 gap-2">
                <Button onClick={() => sessionHub.grant()}>Grant control</Button>
                <Button variant="outline" onClick={() => sessionHub.revoke()}>
                  Revoke
                </Button>
              </div>
              <Button variant="danger" className="mt-2 w-full" onClick={disconnect}>
                Disconnect
              </Button>
            </>
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
}
