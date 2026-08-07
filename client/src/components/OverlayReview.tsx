// Overlay Review — the tangible face of file virtualisation. The workspace works
// on a COPY of a shared folder; here you see exactly what changed (git-style) and
// choose to Merge it back into your real folder or Discard it. Your originals are
// never touched until you say so.
import { useCallback, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { X, FolderPlus, GitMerge, Trash } from "@phosphor-icons/react";
import { Button } from "@/components/ui/Button";
import { cn } from "@/lib/utils";
import {
  overlayChanges,
  overlayDiscard,
  overlayList,
  overlayMerge,
  overlayShare,
  type OverlayChange,
} from "@/lib/wse";
import type { Workspace } from "@/lib/workspace";

const kindMeta: Record<OverlayChange["kind"], { sign: string; tone: string }> = {
  added: { sign: "+", tone: "text-good" },
  modified: { sign: "~", tone: "text-warn" },
  deleted: { sign: "\u2212", tone: "text-danger" },
};

export function OverlayReview({
  workspace,
  onClose,
}: {
  workspace: Workspace;
  onClose: () => void;
}) {
  const [overlays, setOverlays] = useState<string[]>([]);
  const [changes, setChanges] = useState<Record<string, OverlayChange[]>>({});
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    const names = await overlayList(workspace.id);
    setOverlays(names);
    const entries = await Promise.all(
      names.map(async (n) => [n, await overlayChanges(workspace.id, n)] as const),
    );
    setChanges(Object.fromEntries(entries));
  }, [workspace.id]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    await fn();
    await refresh();
    setBusy(false);
  };

  const share = () => {
    if (!path.trim()) return;
    void run(async () => {
      await overlayShare(workspace.id, path.trim());
      setPath("");
    });
  };

  return createPortal(
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      onClick={onClose}
    >
      <div
        className="flex max-h-[85vh] w-full max-w-2xl flex-col overflow-hidden rounded-xl border border-line bg-canvas shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between border-b border-line px-5 py-3.5">
          <div>
            <h2 className="text-[15px] font-semibold">Files — {workspace.name}</h2>
            <p className="mt-0.5 text-[12px] text-muted">
              The workspace works on a copy. Merge applies changes to your real folder; discard
              throws them away. Your originals are untouched until you choose.
            </p>
          </div>
          <button onClick={onClose} className="shrink-0 text-muted transition-colors hover:text-ink">
            <X size={18} />
          </button>
        </div>

        <div className="flex items-center gap-2 border-b border-line px-5 py-3">
          <input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && share()}
            placeholder="C:\\path\\to\\a\\folder to share into this workspace"
            className="min-w-0 flex-1 rounded-md border border-line bg-elevated px-3 py-2 font-mono text-[12.5px] outline-none focus:border-brand"
          />
          <Button size="sm" onClick={share} disabled={busy}>
            <FolderPlus size={14} weight="fill" /> Share
          </Button>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4">
          {overlays.length === 0 ? (
            <p className="py-10 text-center text-[13px] text-muted">
              No shared folders yet. Paste a folder path above and Share it to start.
            </p>
          ) : (
            <div className="space-y-4">
              {overlays.map((name) => {
                const cs = changes[name] ?? [];
                return (
                  <div key={name} className="overflow-hidden rounded-lg border border-line">
                    <div className="flex items-center justify-between gap-2 border-b border-line bg-elevated/40 px-3.5 py-2.5">
                      <div className="min-w-0">
                        <span className="font-mono text-[13px] font-medium">{name}</span>
                        <span className="ml-2 text-[12px] text-muted">
                          {cs.length === 0
                            ? "no changes"
                            : `${cs.length} change${cs.length === 1 ? "" : "s"}`}
                        </span>
                      </div>
                      <div className="flex shrink-0 gap-2">
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => run(() => overlayMerge(workspace.id, name))}
                          disabled={busy || cs.length === 0}
                        >
                          <GitMerge size={13} /> Merge
                        </Button>
                        <Button
                          size="sm"
                          variant="danger"
                          onClick={() => run(() => overlayDiscard(workspace.id, name))}
                          disabled={busy}
                        >
                          <Trash size={13} /> Discard
                        </Button>
                      </div>
                    </div>
                    {cs.length > 0 && (
                      <ul className="max-h-56 divide-y divide-line overflow-y-auto">
                        {cs.map((c) => {
                          const m = kindMeta[c.kind];
                          return (
                            <li
                              key={c.rel}
                              className="flex items-center gap-2 px-3.5 py-1.5 font-mono text-[12px]"
                            >
                              <span className={cn("w-3 shrink-0 text-center font-bold", m.tone)}>
                                {m.sign}
                              </span>
                              <span className="truncate">{c.rel}</span>
                              <span
                                className={cn(
                                  "ml-auto shrink-0 text-[10.5px] uppercase tracking-wide",
                                  m.tone,
                                )}
                              >
                                {c.kind}
                              </span>
                            </li>
                          );
                        })}
                      </ul>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
}
