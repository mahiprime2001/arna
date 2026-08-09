// Guest side of Watch & Control: the workspace surface (video) plus input
// capture. The guest ALWAYS sends input; the host's gate decides whether it's
// accepted — so this view never needs to know the guest's role.
import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { SignOut } from "@phosphor-icons/react";
import { attachInputCapture, sessionHub } from "@/lib/workspace-session";

export function RemoteView({ stream, onLeave }: { stream: MediaStream; onLeave: () => void }) {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const v = videoRef.current;
    if (!v) return;
    v.srcObject = stream;
    v.play().catch(() => {});
    const guest = sessionHub.guestSession();
    const detach = guest ? attachInputCapture(v, guest) : () => {};
    v.focus();
    return () => detach();
  }, [stream]);

  return createPortal(
    <div className="fixed inset-0 z-[60] flex flex-col bg-black">
      <div className="flex items-center justify-between gap-3 px-4 py-2">
        <span className="text-[12.5px] text-white/70">
          Remote workspace — your input is sent; the host controls whether it's accepted.
        </span>
        <button
          onClick={onLeave}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-white/20 px-3 py-1.5 text-[12.5px] text-white/90 transition-colors hover:bg-white/10"
        >
          <SignOut size={14} /> Leave
        </button>
      </div>
      <video
        ref={videoRef}
        className="min-h-0 flex-1 cursor-none bg-black outline-none"
        playsInline
        tabIndex={0}
      />
    </div>,
    document.body,
  );
}
