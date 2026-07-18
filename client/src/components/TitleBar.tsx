import { Minus, Square, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import type { ReactNode } from "react";

// Window controls are wired to the native window when we nativize (Tauri).
// In the browser these are inert.
function windowAction(_action: "min" | "max" | "close") {}

function WinBtn({
  children,
  onClick,
  danger,
}: {
  children: ReactNode;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "grid h-7 w-9 place-items-center rounded text-muted transition-colors",
        danger
          ? "hover:bg-danger hover:text-white"
          : "hover:bg-elevated hover:text-ink",
      )}
    >
      {children}
    </button>
  );
}

export function TitleBar({
  unread,
  onBell,
}: {
  unread: number;
  onBell: () => void;
}) {
  return (
    <header
      data-tauri-drag-region
      className="flex h-11 shrink-0 select-none items-center gap-3 border-b border-line bg-surface px-3"
    >
      <div className="pointer-events-none flex items-center gap-2">
        <div className="grid h-6 w-6 place-items-center rounded-md bg-brand text-[13px] font-bold text-brand-fg">
          A
        </div>
        <span className="text-[13px] font-semibold tracking-tight">Arna</span>
      </div>

      <div className="ml-auto flex items-center gap-1">
        {unread > 0 && (
          <button
            onClick={onBell}
            className="mr-1 h-6 rounded-full bg-brand-soft px-2.5 text-[12px] font-medium text-brand transition-colors hover:bg-brand/20"
          >
            {unread} new
          </button>
        )}
        <WinBtn onClick={() => windowAction("min")}>
          <Minus size={14} weight="bold" />
        </WinBtn>
        <WinBtn onClick={() => windowAction("max")}>
          <Square size={11} weight="bold" />
        </WinBtn>
        <WinBtn danger onClick={() => windowAction("close")}>
          <X size={14} weight="bold" />
        </WinBtn>
      </div>
    </header>
  );
}
