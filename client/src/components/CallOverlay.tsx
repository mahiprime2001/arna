import { useEffect, useRef, useState } from "react";
import {
  Microphone,
  MicrophoneSlash,
  VideoCamera,
  VideoCameraSlash,
  PhoneDisconnect,
  Phone,
  LockSimple,
} from "@phosphor-icons/react";
import { Avatar } from "@/components/Avatar";
import { cn } from "@/lib/utils";
import { callEngine, type CallState } from "@/lib/webrtc";

/**
 * WhatsApp-style call screen.
 *
 * Audio: dark gradient, big avatar, name + status stacked at the top, a row of
 * round translucent controls at the bottom with a red hang-up.
 * Video: remote fills the screen, your own camera sits in a small rounded
 * card in the top corner, the same control row floats over the bottom.
 */

function RoundBtn({
  onClick,
  label,
  tone = "glass",
  size = "md",
  children,
}: {
  onClick: () => void;
  label: string;
  tone?: "glass" | "glass-on" | "end" | "accept";
  size?: "md" | "lg";
  children: React.ReactNode;
}) {
  const tones = {
    glass: "bg-white/15 text-white hover:bg-white/25",
    // "on" is the pressed look WhatsApp uses for an active toggle: filled white.
    "glass-on": "bg-white text-slate-900 hover:bg-white/90",
    end: "bg-call-end text-white hover:brightness-110",
    accept: "bg-call-accept text-white hover:brightness-110",
  }[tone];
  const box = size === "lg" ? "h-16 w-16" : "h-14 w-14";
  return (
    <button
      onClick={onClick}
      aria-label={label}
      className={cn(
        "grid place-items-center rounded-full backdrop-blur-sm transition",
        box,
        tones,
      )}
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
  }, [state.localStream, state.status]);

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
      <div className="fixed inset-0 z-50 grid place-items-center bg-call-bg/95 p-6 backdrop-blur-sm">
        <div className="w-full max-w-sm rounded-2xl bg-call-bg2 p-6 text-center text-white">
          <div className="mx-auto mb-3 grid h-12 w-12 place-items-center rounded-full bg-call-end/20 text-call-end">
            <PhoneDisconnect size={22} weight="fill" />
          </div>
          <h2 className="text-base font-semibold">Call couldn't start</h2>
          <p className="mt-1.5 text-sm text-white/60">{state.error}</p>
          <button
            onClick={() => callEngine.dismissError()}
            className="mt-5 rounded-full bg-white/15 px-6 py-2.5 text-sm font-semibold transition hover:bg-white/25"
          >
            Close
          </button>
        </div>
      </div>
    );
  }

  const isVideo = state.kind === "video";
  const connected = state.status === "connected";
  const incoming = state.status === "incoming";
  const showRemoteVideo = isVideo && connected;
  const clock = `${String(Math.floor(secs / 60)).padStart(2, "0")}:${String(secs % 60).padStart(2, "0")}`;

  const statusLine = incoming
    ? `Incoming ${isVideo ? "video" : "voice"} call`
    : state.status === "outgoing"
      ? "Ringing…"
      : state.status === "connecting"
        ? "Connecting…"
        : clock;

  return (
    <div className="fixed inset-0 z-50 select-none overflow-hidden bg-gradient-to-b from-call-bg2 to-call-bg text-white">
      {/* Remote video fills the screen on a connected video call. It stays
          mounted (hidden) on voice calls so the remote audio keeps playing. */}
      <video
        ref={remoteRef}
        autoPlay
        playsInline
        className={cn(
          showRemoteVideo ? "absolute inset-0 h-full w-full object-cover" : "hidden",
        )}
      />
      {showRemoteVideo && (
        // Scrim so the name and the controls stay legible over any footage.
        <div className="absolute inset-0 bg-gradient-to-b from-black/55 via-transparent to-black/65" />
      )}

      {/* Name + status. Centred on a voice call; tucked top-left once video is
          carrying the screen, so it doesn't cover the other person's face. */}
      <div
        className={cn(
          "absolute z-10",
          showRemoteVideo ? "left-6 top-6 text-left" : "inset-x-0 top-0 pt-16 text-center",
        )}
      >
        <p className={cn("font-medium", showRemoteVideo ? "text-lg" : "text-[26px]")}>
          {state.peerName || "Unknown"}
        </p>
        <p
          className={cn(
            "mt-1 text-white/70",
            showRemoteVideo ? "text-[13px] tabular-nums" : "text-[15px] tabular-nums",
          )}
        >
          {statusLine}
        </p>
        {!showRemoteVideo && (
          <p className="mt-5 inline-flex items-center gap-1.5 text-[11.5px] text-white/45">
            <LockSimple size={12} weight="fill" />
            End-to-end encrypted
          </p>
        )}
      </div>

      {/* Voice call (or video that hasn't connected yet): the avatar carries it. */}
      {!showRemoteVideo && (
        <div className="absolute inset-0 grid place-items-center">
          <div className="relative">
            {(state.status === "outgoing" || incoming) && (
              <span className="absolute -inset-3 animate-ping rounded-full bg-white/10" />
            )}
            <Avatar name={state.peerName || "?"} size={148} />
          </div>
        </div>
      )}

      {/* Your own camera, as a small rounded card in the corner. */}
      {isVideo && state.localStream && !state.camOff && (
        <video
          ref={localRef}
          autoPlay
          playsInline
          muted
          className={cn(
            "absolute z-10 rounded-2xl object-cover shadow-pop ring-1 ring-white/20",
            showRemoteVideo ? "right-5 top-5 h-44 w-32" : "bottom-36 right-5 h-40 w-28",
          )}
        />
      )}

      {/* Controls */}
      <div className="absolute inset-x-0 bottom-0 z-10 pb-14">
        {incoming ? (
          <div className="flex items-center justify-center gap-24">
            <div className="flex flex-col items-center gap-2.5">
              <RoundBtn onClick={() => callEngine.decline()} label="Decline" tone="end" size="lg">
                <PhoneDisconnect size={26} weight="fill" />
              </RoundBtn>
              <span className="text-[12.5px] text-white/60">Decline</span>
            </div>
            <div className="flex flex-col items-center gap-2.5">
              <RoundBtn onClick={() => callEngine.accept()} label="Accept" tone="accept" size="lg">
                <Phone size={26} weight="fill" />
              </RoundBtn>
              <span className="text-[12.5px] text-white/60">Accept</span>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-center gap-5">
            <RoundBtn
              onClick={() => callEngine.toggleMic()}
              label={state.muted ? "Unmute" : "Mute"}
              tone={state.muted ? "glass-on" : "glass"}
            >
              {state.muted ? <MicrophoneSlash size={23} /> : <Microphone size={23} />}
            </RoundBtn>

            {isVideo && (
              <RoundBtn
                onClick={() => callEngine.toggleCam()}
                label={state.camOff ? "Turn camera on" : "Turn camera off"}
                tone={state.camOff ? "glass-on" : "glass"}
              >
                {state.camOff ? <VideoCameraSlash size={23} /> : <VideoCamera size={23} />}
              </RoundBtn>
            )}

            <RoundBtn onClick={() => callEngine.hangup()} label="End call" tone="end" size="lg">
              <PhoneDisconnect size={26} weight="fill" />
            </RoundBtn>
          </div>
        )}
      </div>
    </div>
  );
}
