import { useEffect, useRef, useState } from "react";
import {
  PaperPlaneTilt,
  Paperclip,
  Microphone,
  Trash,
  Stop,
  Check,
  Checks,
  File as FileIcon,
  DownloadSimple,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/Button";
import { cn } from "@/lib/utils";
import type { ChatMessage, OutgoingPayload } from "@/lib/mock";

// ── file / media helpers ────────────────────────────────────────────────────
function readDataUrl(file: Blob): Promise<string> {
  return new Promise((res, rej) => {
    const r = new FileReader();
    r.onload = () => res(r.result as string);
    r.onerror = rej;
    r.readAsDataURL(file);
  });
}
function loadImg(src: string): Promise<HTMLImageElement> {
  return new Promise((res, rej) => {
    const i = new Image();
    i.onload = () => res(i);
    i.onerror = rej;
    i.src = src;
  });
}
async function fileToPayload(file: File): Promise<OutgoingPayload> {
  if (file.type.startsWith("image/")) {
    const url = await readDataUrl(file);
    try {
      const img = await loadImg(url);
      const max = 1280;
      const scale = Math.min(1, max / Math.max(img.width, img.height));
      const w = Math.round(img.width * scale);
      const h = Math.round(img.height * scale);
      const canvas = document.createElement("canvas");
      canvas.width = w;
      canvas.height = h;
      canvas.getContext("2d")!.drawImage(img, 0, 0, w, h);
      const out = canvas.toDataURL("image/jpeg", 0.82);
      return { kind: "image", media: { data: out, mime: "image/jpeg", w, h, size: out.length } };
    } catch {
      return { kind: "image", media: { data: url, mime: file.type, size: file.size } };
    }
  }
  const data = await readDataUrl(file);
  return {
    kind: "file",
    media: {
      data,
      mime: file.type || "application/octet-stream",
      name: file.name,
      size: file.size,
    },
  };
}
function fmtBytes(n?: number): string {
  if (!n) return "";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

function Tick({ status }: { status?: "sent" | "delivered" | "read" }) {
  if (status === "sent") return <Check size={13} weight="bold" className="text-brand-fg/50" />;
  if (status === "delivered") return <Checks size={14} weight="bold" className="text-brand-fg/55" />;
  if (status === "read") return <Checks size={14} weight="bold" className="text-white" />;
  return null;
}

function Bubble({ m }: { m: ChatMessage }) {
  const mine = m.mine;
  const meta = (
    <div className="mt-1 flex items-center justify-end gap-1">
      <span className={cn("text-[10.5px]", mine ? "text-brand-fg/70" : "text-muted")}>
        {m.time}
      </span>
      {mine && <Tick status={m.status} />}
    </div>
  );
  return (
    <div className={cn("flex", mine ? "justify-end" : "justify-start")}>
      <div
        className={cn(
          "max-w-[80%] overflow-hidden text-sm",
          mine ? "rounded-2xl rounded-br-md bg-brand text-brand-fg" : "rounded-2xl rounded-bl-md bg-elevated text-ink",
        )}
      >
        {m.kind === "image" && m.media && (
          <img
            src={m.media.data}
            alt=""
            onClick={() => window.open(m.media!.data, "_blank")}
            className="block max-h-72 w-full max-w-[260px] cursor-pointer object-cover"
          />
        )}
        {m.kind === "audio" && m.media && (
          <div className="px-2 pt-2">
            <audio controls src={m.media.data} className="h-9 w-56 max-w-full" />
          </div>
        )}
        {m.kind === "file" && m.media && (
          <a
            href={m.media.data}
            download={m.media.name || "file"}
            className={cn(
              "flex items-center gap-3 px-3 pt-3",
              mine ? "hover:opacity-90" : "hover:bg-elevated",
            )}
          >
            <span
              className={cn(
                "grid h-9 w-9 shrink-0 place-items-center rounded-lg",
                mine ? "bg-black/15" : "bg-canvas",
              )}
            >
              <FileIcon size={18} />
            </span>
            <div className="min-w-0 flex-1">
              <p className="truncate font-medium">{m.media.name || "File"}</p>
              <p className={cn("text-[12px]", mine ? "text-brand-fg/70" : "text-muted")}>
                {fmtBytes(m.media.size)}
              </p>
            </div>
            <DownloadSimple size={16} className="shrink-0" />
          </a>
        )}

        <div className="px-3.5 pb-2 pt-2">
          {m.text && <p className="whitespace-pre-wrap break-words">{m.text}</p>}
          {meta}
        </div>
      </div>
    </div>
  );
}

// ── the chat surface ────────────────────────────────────────────────────────
export function Chat({
  messages,
  onSend,
  placeholder = "Message",
}: {
  messages: ChatMessage[];
  onSend: (payload: OutgoingPayload) => void;
  placeholder?: string;
}) {
  const [text, setText] = useState("");
  const [recSecs, setRecSecs] = useState(0);
  const [recording, setRecording] = useState(false);

  const endRef = useRef<HTMLDivElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const recRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const streamRef = useRef<MediaStream | null>(null);
  const timerRef = useRef<number | undefined>(undefined);
  const secsRef = useRef(0);
  const cancelledRef = useRef(false);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length]);

  const sendText = () => {
    const t = text.trim();
    if (!t) return;
    onSend({ kind: "text", text: t });
    setText("");
  };

  const onFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (file) onSend(await fileToPayload(file));
  };

  const startRec = async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      streamRef.current = stream;
      const mr = new MediaRecorder(stream);
      chunksRef.current = [];
      cancelledRef.current = false;
      mr.ondataavailable = (e) => e.data.size && chunksRef.current.push(e.data);
      mr.onstop = async () => {
        streamRef.current?.getTracks().forEach((t) => t.stop());
        if (cancelledRef.current) return;
        const blob = new Blob(chunksRef.current, { type: mr.mimeType || "audio/webm" });
        const data = await readDataUrl(blob);
        onSend({
          kind: "audio",
          media: { data, mime: blob.type, dur: secsRef.current, size: blob.size },
        });
      };
      mr.start();
      recRef.current = mr;
      secsRef.current = 0;
      setRecSecs(0);
      setRecording(true);
      timerRef.current = window.setInterval(() => {
        secsRef.current += 1;
        setRecSecs(secsRef.current);
      }, 1000);
    } catch {
      /* mic unavailable or denied */
    }
  };
  const stopRec = (cancel: boolean) => {
    cancelledRef.current = cancel;
    clearInterval(timerRef.current);
    setRecording(false);
    recRef.current?.stop();
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 space-y-2.5 overflow-y-auto p-4">
        {messages.length === 0 && (
          <div className="grid h-full place-items-center">
            <p className="text-sm text-muted">No messages yet. Say hello.</p>
          </div>
        )}
        {messages.map((m) => (
          <Bubble key={m.id} m={m} />
        ))}
        <div ref={endRef} />
      </div>

      <div className="flex items-center gap-2 border-t border-line p-3">
        {recording ? (
          <div className="flex flex-1 items-center gap-3 rounded-lg bg-elevated px-3 py-2">
            <span className="h-2.5 w-2.5 animate-pulse rounded-full bg-danger" />
            <span className="text-sm tabular-nums text-muted">
              Recording {String(Math.floor(recSecs / 60)).padStart(2, "0")}:
              {String(recSecs % 60).padStart(2, "0")}
            </span>
            <button
              onClick={() => stopRec(true)}
              aria-label="Cancel recording"
              className="ml-auto grid h-9 w-9 place-items-center rounded-lg text-muted transition-colors hover:bg-danger/10 hover:text-danger"
            >
              <Trash size={18} />
            </button>
            <Button
              size="icon"
              onClick={() => stopRec(false)}
              aria-label="Send voice message"
              className="h-9 w-9 rounded-lg"
            >
              <Stop size={16} weight="fill" />
            </Button>
          </div>
        ) : (
          <>
            <button
              onClick={() => fileRef.current?.click()}
              aria-label="Attach a file"
              className="grid h-10 w-10 shrink-0 place-items-center rounded-lg text-muted transition-colors hover:bg-elevated hover:text-ink"
            >
              <Paperclip size={19} />
            </button>
            <input ref={fileRef} type="file" hidden onChange={onFile} />
            <input
              value={text}
              onChange={(e) => setText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  sendText();
                }
              }}
              placeholder={placeholder}
              className="h-10 flex-1 rounded-lg border border-line bg-canvas px-3.5 text-sm outline-none transition-colors placeholder:text-muted/70 focus:border-brand/50 focus:ring-2 focus:ring-brand/25"
            />
            {text.trim() ? (
              <Button size="icon" onClick={sendText} aria-label="Send" className="h-10 w-10 rounded-lg">
                <PaperPlaneTilt size={17} weight="fill" />
              </Button>
            ) : (
              <button
                onClick={startRec}
                aria-label="Record voice message"
                className="grid h-10 w-10 shrink-0 place-items-center rounded-lg text-muted transition-colors hover:bg-elevated hover:text-ink"
              >
                <Microphone size={19} />
              </button>
            )}
          </>
        )}
      </div>
    </div>
  );
}
