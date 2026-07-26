import { useState } from "react";
import {
  Globe,
  Terminal as TerminalIcon,
  FolderOpen,
  Code,
  X,
  SquaresFour,
  Columns,
  Square,
  ArrowsOutSimple,
  Spinner,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import type { Workspace } from "@/lib/workspace";

// The apps the guest can open in the workspace. Each launches inside the sealed
// VM and appears as a window on the canvas. Wine .exe apps join this list later.
interface StageApp {
  id: string;
  name: string;
  icon: React.ReactNode;
}
const STAGE_APPS: StageApp[] = [
  { id: "browser", name: "Browser", icon: <Globe size={22} /> },
  { id: "files", name: "Files", icon: <FolderOpen size={22} /> },
  { id: "terminal", name: "Terminal", icon: <TerminalIcon size={22} /> },
  { id: "editor", name: "Editor", icon: <Code size={22} /> },
];

// How the open windows are arranged. The snapping itself is done by the
// workspace's own window manager, so it behaves like Windows Snap; these are
// the quick-layout shortcuts.
type Layout = "free" | "split" | "grid" | "full";

const LAYOUTS: { id: Layout; label: string; icon: React.ReactNode }[] = [
  { id: "full", label: "Full", icon: <Square size={16} /> },
  { id: "split", label: "Side by side", icon: <Columns size={16} /> },
  { id: "grid", label: "2×2 grid", icon: <SquaresFour size={16} /> },
  { id: "free", label: "Free drag", icon: <ArrowsOutSimple size={16} /> },
];

// Where the workspace canvas is streamed from. Same-origin in production (the
// backend proxies the VM's noVNC); overridable for local testing.
const streamBase =
  (import.meta.env.VITE_ARNA_STREAM as string | undefined) ?? "http://localhost:6080";

export function WorkspaceStage({
  workspace,
  onClose,
  onLaunch,
  onLayout,
}: {
  workspace: Workspace;
  onClose: () => void;
  onLaunch: (appId: string) => void;
  onLayout: (layout: Layout) => void;
}) {
  const [layout, setLayout] = useState<Layout>("grid");
  const [loading, setLoading] = useState(true);
  const [launching, setLaunching] = useState<string | null>(null);

  const streamUrl = `${streamBase}/vnc_lite.html?resize=scale&autoconnect=1&reconnect=1`;

  const launch = (id: string) => {
    setLaunching(id);
    onLaunch(id);
    // The window appears in the stream a moment later; clear the busy state.
    window.setTimeout(() => setLaunching(null), 1500);
  };

  const setLay = (l: Layout) => {
    setLayout(l);
    onLayout(l);
  };

  return (
    <div className="fixed inset-0 z-40 flex bg-slate-950 text-ink">
      {/* App launcher rail */}
      <aside className="flex w-20 shrink-0 flex-col items-center gap-1 border-r border-line bg-surface py-3">
        <div className="mb-2 grid h-10 w-10 place-items-center rounded-xl bg-brand text-brand-fg">
          <SquaresFour size={20} weight="fill" />
        </div>
        {STAGE_APPS.map((a) => (
          <button
            key={a.id}
            onClick={() => launch(a.id)}
            title={`Open ${a.name}`}
            className="group flex w-full flex-col items-center gap-1 rounded-lg px-1 py-2 text-muted transition-colors hover:bg-elevated hover:text-ink"
          >
            <span className="grid h-11 w-11 place-items-center rounded-xl bg-elevated transition-colors group-hover:bg-canvas">
              {launching === a.id ? <Spinner size={22} className="animate-spin" /> : a.icon}
            </span>
            <span className="text-[10.5px]">{a.name}</span>
          </button>
        ))}
      </aside>

      {/* Stage */}
      <div className="flex min-w-0 flex-1 flex-col">
        {/* Top bar: name + layout controls */}
        <header className="flex items-center gap-3 border-b border-line bg-surface px-4 py-2">
          <div className="min-w-0">
            <p className="truncate text-sm font-semibold">{workspace.name}</p>
            <p className="text-[11.5px] text-muted">Running · sealed workspace</p>
          </div>

          <div className="ml-auto flex items-center gap-1 rounded-lg bg-elevated p-0.5">
            {LAYOUTS.map((l) => (
              <button
                key={l.id}
                onClick={() => setLay(l.id)}
                title={l.label}
                className={cn(
                  "flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[12px] transition-colors",
                  layout === l.id ? "bg-surface text-ink shadow-sm" : "text-muted hover:text-ink",
                )}
              >
                {l.icon}
                <span className="hidden sm:inline">{l.label}</span>
              </button>
            ))}
          </div>

          <button
            onClick={onClose}
            aria-label="Leave workspace"
            className="grid h-9 w-9 place-items-center rounded-lg text-muted transition-colors hover:bg-danger/10 hover:text-danger"
          >
            <X size={18} />
          </button>
        </header>

        {/* The workspace canvas (streamed from the sealed VM) */}
        <div className="relative flex-1 overflow-hidden bg-black">
          {loading && (
            <div className="absolute inset-0 z-10 grid place-items-center bg-slate-950">
              <div className="flex flex-col items-center gap-3 text-muted">
                <Spinner size={30} className="animate-spin text-brand" />
                <p className="text-sm">Connecting to the workspace…</p>
              </div>
            </div>
          )}
          <iframe
            title={`${workspace.name} workspace`}
            src={streamUrl}
            onLoad={() => setLoading(false)}
            className="h-full w-full border-0"
            allow="clipboard-read; clipboard-write"
          />
        </div>

        {/* Hint bar */}
        <footer className="border-t border-line bg-surface px-4 py-1.5 text-[11.5px] text-muted">
          Open apps from the left. Drag a window to an edge to snap it side-by-side, or use
          the layout buttons to tile them.
        </footer>
      </div>
    </div>
  );
}
