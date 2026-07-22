import { useEffect, useRef, useState } from "react";
import {
  Microphone,
  MicrophoneSlash,
  VideoCamera,
  VideoCameraSlash,
  PhoneX,
  Phone,
} from "@phosphor-icons/react";
import { Avatar } from "@/components/Avatar";
import { cn } from "@/lib/utils";
import { callEngine, type CallState } from "@/lib/webrtc";

function RoundBtn({
  onClick,
  label,
  tone = "light",
  children,
}: {
  onClick: () => void;
  label: string;
  tone?: "light" | "dim" | "danger" | "good";
  children: React.ReactNode;
}) {
  const styles = {
    light: "bg-white text-slate-900 hover:bg-white/90",
    dim: "bg-white/10 text-white hover:bg-white/20",
    danger: "bg-danger text-white hover:brightness-110",
    good: "bg-good text-white hover:brightness-110",
  }[tone];
  return (
    <button
      onClick={onClick}
      aria-label={label}
      className={cn("grid h-14 w-14 place-items-center rounded-full transition", styles)}
    >
      {children}
    </button>
  );
}

export function CallOverlay({ state }: { state: CallState }) {
  const remoteRef = useRef<HTMLVideoElement>(null);
  const localRef = useRef<HTMLVideoElement>(null);
  const [secs, setSecs] = useState(0);

  useEffect(() => {
    if (remoteRef.current) remoteRef.current.srcObject = state.remoteStream;
  }, [state.remoteStream, state.status, state.kind]);
  useEffect(() => {
    if (localRef.current) localRef.current.srcObject = state.localStream;
  }, [state.localStream]);

  useEffect(() => {
    if (state.status !== "connected") {
      setSecs(0);
      return;
    }
    const id = setInterval(() => setSecs((s) => s + 1), 1000);
    return () => clearInterval(id);
  }, [state.status]);

  if (state.status === "idle" && !state.error) return null;

  if (state.error) {
    return (
      <div className="fixed inset-0 z-50 grid place-items-center bg-slate-950/95 p-6 backdrop-blur-sm">
        <div className="w-full max-w-sm rounded-xl border border-line bg-surface p-6 text-center text-ink">
          <div className="mx-auto mb-3 grid h-12 w-12 place-items-center rounded-full bg-danger/15 text-danger">
            <PhoneX size={22} weight="fill" />
          </div>
          <h2 className="text-base font-semibold">Call couldn't start</h2>
          <p className="mt-1.5 text-sm text-muted">{state.error}</p>
          <button
            onClick={() => callEngine.dismissError()}
            className="mt-4 rounded-lg bg-brand px-5 py-2 text-sm font-semibold text-brand-fg transition hover:brightness-110"
          >
            Close
          </button>
        </div>
      </div>
    );
  }

  const isVideo = state.kind === "video";
  const connected = state.status === "connected";
  const showRemoteVideo = isVideo && connected;
  const clock = `${String(Math.floor(secs / 60)).padStart(2, "0")}:${String(secs % 60).padStart(2, "0")}`;

  const statusLine =
    state.status === "outgoing"
      ? "Calling"
      : state.status === "incoming"
        ? `Incoming ${isVideo ? "video" : "voice"} call`
        : `${isVideo ? "Video" : "Voice"} call · ${clock}`;

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-slate-950/95 backdrop-blur-sm">
      <div className="relative flex flex-1 items-center justify-center overflow-hidden">
        {/* Remote media: visible for connected video, otherwise hidden (audio still plays). */}
        <video
          ref={remoteRef}
          autoPlay
          playsInline
          className={cn(showRemoteVideo ? "h-full w-full object-contain" : "hidden")}
        />

        {!showRemoteVideo && (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-4">
            <Avatar name={state.peerName || "?"} size={112} />
            <div className="text-center">
              <p className="text-xl font-semibold text-white">{state.peerName || "Unknown"}</p>
              <p className="mt-1 text-sm text-slate-400">{statusLine}</p>
            </div>
          </div>
        )}

        {/* Local self-preview (video calls). */}
        {isVideo && state.localStream && (
          <video
            ref={localRef}
            autoPlay
            playsInline
            muted
            className="absolute bottom-6 right-6 h-32 w-48 rounded-xl border border-white/10 object-cover"
          />
        )}
      </div>

      <div className="flex items-center justify-center gap-4 pb-10">
        {state.status === "incoming" ? (
          <>
            <RoundBtn onClick={() => callEngine.decline()} label="Decline" tone="danger">
              <PhoneX size={24} weight="fill" />
            </RoundBtn>
            <RoundBtn onClick={() => callEngine.accept()} label="Accept" tone="good">
              <Phone size={24} weight="fill" />
            </RoundBtn>
          </>
        ) : (
          <>
            <RoundBtn
              onClick={() => callEngine.toggleMic()}
              label="Toggle mic"
              tone={state.muted ? "dim" : "light"}
            >
              {state.muted ? <MicrophoneSlash size={22} /> : <Microphone size={22} />}
            </RoundBtn>
            {isVideo && (
              <RoundBtn
                onClick={() => callEngine.toggleCam()}
                label="Toggle camera"
                tone={state.camOff ? "dim" : "light"}
              >
                {state.camOff ? <VideoCameraSlash size={22} /> : <VideoCamera size={22} />}
              </RoundBtn>
            )}
            <RoundBtn onClick={() => callEngine.hangup()} label="End call" tone="danger">
              <PhoneX size={24} weight="fill" />
            </RoundBtn>
          </>
        )}
      </div>
    </div>
  );
}
