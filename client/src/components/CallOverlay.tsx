import { useEffect, useState, type ReactNode } from "react";
import {
  Microphone,
  MicrophoneSlash,
  VideoCamera,
  VideoCameraSlash,
  PhoneX,
} from "@phosphor-icons/react";
import { Avatar } from "@/components/Avatar";
import { cn } from "@/lib/utils";

export type Call = { name: string; kind: "audio" | "video" };

function CtlButton({
  active,
  onClick,
  children,
  label,
}: {
  active: boolean;
  onClick: () => void;
  children: ReactNode;
  label: string;
}) {
  return (
    <button
      onClick={onClick}
      aria-label={label}
      className={cn(
        "grid h-14 w-14 place-items-center rounded-full transition",
        active
          ? "bg-white/10 text-white hover:bg-white/20"
          : "bg-white text-slate-900 hover:bg-white/90",
      )}
    >
      {children}
    </button>
  );
}

// Mock call surface (no real media yet). Real WebRTC P2P wiring comes next.
export function CallOverlay({ call, onEnd }: { call: Call; onEnd: () => void }) {
  const [muted, setMuted] = useState(false);
  const [camOff, setCamOff] = useState(call.kind === "audio");
  const [secs, setSecs] = useState(0);

  useEffect(() => {
    const id = setInterval(() => setSecs((s) => s + 1), 1000);
    return () => clearInterval(id);
  }, []);

  const clock = `${String(Math.floor(secs / 60)).padStart(2, "0")}:${String(
    secs % 60,
  ).padStart(2, "0")}`;

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-slate-950/95 backdrop-blur-sm">
      <div className="relative flex flex-1 items-center justify-center overflow-hidden">
        {call.kind === "video" && !camOff ? (
          <div className="grid h-full w-full place-items-center bg-gradient-to-br from-slate-800 to-slate-950">
            <div className="flex flex-col items-center gap-3">
              <Avatar name={call.name} size={96} />
              <p className="text-sm text-slate-400">{call.name}'s camera</p>
            </div>
          </div>
        ) : (
          <div className="flex flex-col items-center gap-4">
            <Avatar name={call.name} size={112} />
            <div className="text-center">
              <p className="text-xl font-semibold text-white">{call.name}</p>
              <p className="mt-1 text-sm text-slate-400">
                {call.kind === "audio" ? "Voice call" : "Camera off"} · {clock}
              </p>
            </div>
          </div>
        )}

        {call.kind === "video" && (
          <div className="absolute bottom-6 right-6 grid h-32 w-48 place-items-center rounded-xl border border-white/10 bg-slate-800/80 text-xs text-slate-400">
            {camOff ? "You · camera off" : "You"}
          </div>
        )}
      </div>

      <div className="flex items-center justify-center gap-4 pb-10">
        <CtlButton active={muted} onClick={() => setMuted((m) => !m)} label="Toggle mic">
          {muted ? <MicrophoneSlash size={22} /> : <Microphone size={22} />}
        </CtlButton>
        {call.kind === "video" && (
          <CtlButton
            active={camOff}
            onClick={() => setCamOff((c) => !c)}
            label="Toggle camera"
          >
            {camOff ? <VideoCameraSlash size={22} /> : <VideoCamera size={22} />}
          </CtlButton>
        )}
        <button
          onClick={onEnd}
          aria-label="End call"
          className="grid h-14 w-14 place-items-center rounded-full bg-danger text-white transition hover:brightness-110"
        >
          <PhoneX size={24} weight="fill" />
        </button>
      </div>
    </div>
  );
}
