import { useEffect, useMemo, useRef, useState } from "react";
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
  ArrowBendUpLeft,
  ArrowBendUpRight,
  PencilSimple,
  PushPin,
  Copy,
  SmileySticker,
  Smiley,
  DotsThree,
  Timer,
  X,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/Button";
import { cn } from "@/lib/utils";
import { dayLabel, summarize, untilLabel } from "@/lib/chat";
import { QUICK_REACTIONS, type ChatMessage, type OutgoingPayload } from "@/lib/mock";

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

// A compact emoji palette for the composer. Enough to be useful without
// pulling in a multi-megabyte picker library.
const EMOJI = [
  "😀","😄","😁","😂","🤣","😊","😍","😘","😉","😎",
  "🤔","🙄","😴","😢","😭","😡","🥺","😱","🤯","🤗",
  "👍","👎","👏","🙌","🙏","💪","👀","🔥","✨","🎉",
  "❤️","🧡","💛","💚","💙","💜","🖤","💔","💯","✅",
  "😅","😇","🥳","🤝","👋","🤞","☕","🍕","🚀","⭐",
];

function EmojiPicker({ onPick, onClose }: { onPick: (e: string) => void; onClose: () => void }) {
  return (
    <>
      <div className="fixed inset-0 z-20" onClick={onClose} />
      <div className="absolute bottom-full left-0 z-30 mb-2 w-[19rem] rounded-xl bg-surface p-2 shadow-pop">
        <div className="grid grid-cols-10 gap-0.5">
          {EMOJI.map((e) => (
            <button
              key={e}
              onClick={() => onPick(e)}
              className="grid h-7 w-7 place-items-center rounded-md text-[17px] transition hover:bg-elevated"
            >
              {e}
            </button>
          ))}
        </div>
      </div>
    </>
  );
}

function Tick({ status }: { status?: "sent" | "delivered" | "read" }) {
  if (status === "sent") return <Check size={13} weight="bold" className="opacity-50" />;
  if (status === "delivered") return <Checks size={14} weight="bold" className="opacity-60" />;
  if (status === "read") return <Checks size={14} weight="bold" className="text-tick" />;
  return null;
}

export interface MsgActions {
  onReply: (m: ChatMessage) => void;
  onEdit: (m: ChatMessage) => void;
  onForward: (m: ChatMessage) => void;
  onPin: (m: ChatMessage) => void;
  onDelete: (m: ChatMessage, forEveryone: boolean) => void;
  onReact: (m: ChatMessage, emoji: string | null) => void;
}

// ── one message ─────────────────────────────────────────────────────────────
function Bubble({
  m,
  actions,
  highlight,
  onJumpTo,
}: {
  m: ChatMessage;
  actions: MsgActions;
  highlight?: boolean;
  onJumpTo?: (mid: string) => void;
}) {
  const mine = m.mine;
  const [menu, setMenu] = useState(false);
  const [picker, setPicker] = useState(false);

  // Reactions in a 1:1 chat: at most one from each side.
  const reactions = [
    ...(m.myReaction ? [{ emoji: m.myReaction, mine: true }] : []),
    ...(m.theirReaction ? [{ emoji: m.theirReaction, mine: false }] : []),
  ];

  const close = () => {
    setMenu(false);
    setPicker(false);
  };

  // These sit in the row's flow next to the bubble. They must NOT be positioned
  // outside the row (right-full/left-full) -- the message list clips its
  // overflow, so anything placed there is invisible.
  const actionBar = (
    <div className="flex shrink-0 items-center gap-0.5 pb-1 opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100">
      <button
        onClick={() => setPicker((v) => !v)}
        aria-label="React"
        className="grid h-7 w-7 place-items-center rounded-full text-muted transition hover:bg-elevated hover:text-ink"
      >
        <SmileySticker size={16} />
      </button>
      <button
        onClick={() => actions.onReply(m)}
        aria-label="Reply"
        className="grid h-7 w-7 place-items-center rounded-full text-muted transition hover:bg-elevated hover:text-ink"
      >
        <ArrowBendUpLeft size={16} />
      </button>
      <button
        onClick={() => setMenu((v) => !v)}
        aria-label="More actions"
        className="grid h-7 w-7 place-items-center rounded-full text-muted transition hover:bg-elevated hover:text-ink"
      >
        <DotsThree size={18} weight="bold" />
      </button>
    </div>
  );

  return (
    <div
      className={cn(
        "group relative flex items-end gap-1 px-1",
        mine ? "justify-end" : "justify-start",
        highlight && "rounded-lg bg-brand/15",
      )}
      onContextMenu={(e) => {
        e.preventDefault();
        setMenu((v) => !v);
      }}
    >
      {mine && actionBar}

      {picker && (
        <>
          <div className="fixed inset-0 z-20" onClick={close} />
          <div
            className={cn(
              "absolute top-8 z-30 flex gap-1 rounded-full bg-surface p-1.5 shadow-pop",
              mine ? "right-2" : "left-2",
            )}
          >
            {QUICK_REACTIONS.map((e) => (
              <button
                key={e}
                onClick={() => {
                  actions.onReact(m, m.myReaction === e ? null : e);
                  close();
                }}
                className={cn(
                  "grid h-8 w-8 place-items-center rounded-full text-[17px] transition hover:bg-elevated",
                  m.myReaction === e && "bg-brand/20",
                )}
              >
                {e}
              </button>
            ))}
          </div>
        </>
      )}

      {menu && (
        <>
          <div className="fixed inset-0 z-20" onClick={close} />
          <div
            className={cn(
              "absolute top-8 z-30 w-52 overflow-hidden rounded-xl bg-surface py-1 text-sm shadow-pop",
              mine ? "right-2" : "left-2",
            )}
          >
            {[
              {
                label: "Reply",
                icon: <ArrowBendUpLeft size={16} />,
                run: () => actions.onReply(m),
              },
              ...(mine && m.kind === "text"
                ? [{ label: "Edit", icon: <PencilSimple size={16} />, run: () => actions.onEdit(m) }]
                : []),
              {
                label: "Forward",
                icon: <ArrowBendUpRight size={16} />,
                run: () => actions.onForward(m),
              },
              { label: "Pin", icon: <PushPin size={16} />, run: () => actions.onPin(m) },
              ...(m.text
                ? [
                    {
                      label: "Copy text",
                      icon: <Copy size={16} />,
                      run: () => navigator.clipboard?.writeText(m.text!),
                    },
                  ]
                : []),
            ].map((it) => (
              <button
                key={it.label}
                onClick={() => {
                  it.run();
                  close();
                }}
                className="flex w-full items-center gap-2.5 px-3 py-2 text-left transition-colors hover:bg-elevated"
              >
                <span className="text-muted">{it.icon}</span>
                {it.label}
              </button>
            ))}
            <div className="my-1 h-px bg-line" />
            <button
              onClick={() => {
                actions.onDelete(m, false);
                close();
              }}
              className="flex w-full items-center gap-2.5 px-3 py-2 text-left text-danger transition-colors hover:bg-danger/10"
            >
              <Trash size={16} /> Delete for me
            </button>
            {mine && (
              <button
                onClick={() => {
                  actions.onDelete(m, true);
                  close();
                }}
                className="flex w-full items-center gap-2.5 px-3 py-2 text-left text-danger transition-colors hover:bg-danger/10"
              >
                <Trash size={16} weight="fill" /> Delete for everyone
              </button>
            )}
          </div>
        </>
      )}

      <div className="max-w-[80%]">
        <div
          className={cn(
            "overflow-hidden text-sm",
            mine
              ? "rounded-2xl rounded-br-md bg-bubble-out text-bubble-out-fg"
              : "rounded-2xl rounded-bl-md bg-bubble-in text-bubble-in-fg",
          )}
        >
          {m.fwdFrom && (
            <p className="px-3.5 pt-2 text-[12px] text-brand">
              Forwarded from <span className="font-medium">{m.fwdFrom}</span>
            </p>
          )}

          {m.replyTo && (
            <button
              onClick={() => onJumpTo?.(m.replyTo!)}
              className="mx-2 mt-2 flex w-[calc(100%-1rem)] gap-2 rounded-md bg-black/5 px-2 py-1.5 text-left dark:bg-white/10"
            >
              <span className="w-0.5 shrink-0 rounded-full bg-brand" />
              <span className="min-w-0">
                <span className="block text-[12px] font-medium text-brand">{m.replyAuthor}</span>
                <span className="block truncate text-[12px] opacity-70">{m.replyPreview}</span>
              </span>
            </button>
          )}

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
              className="flex items-center gap-3 px-3 pt-3 hover:opacity-90"
            >
              <span className="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-black/10 dark:bg-white/10">
                <FileIcon size={18} />
              </span>
              <div className="min-w-0 flex-1">
                <p className="truncate font-medium">{m.media.name || "File"}</p>
                <p className="text-[12px] opacity-70">{fmtBytes(m.media.size)}</p>
              </div>
              <DownloadSimple size={16} className="shrink-0" />
            </a>
          )}

          <div className="px-3.5 pb-1.5 pt-2">
            {m.text && <p className="whitespace-pre-wrap break-words">{m.text}</p>}
            <div className="mt-1 flex items-center justify-end gap-1">
              {m.expiresAt && (
                <span className="flex items-center gap-0.5 text-[10.5px] opacity-60">
                  <Timer size={11} />
                  {untilLabel(m.expiresAt)}
                </span>
              )}
              {m.edited && <span className="text-[10.5px] opacity-60">edited</span>}
              <span className="text-[10.5px] opacity-60">{m.time}</span>
              {mine && <Tick status={m.status} />}
            </div>
          </div>
        </div>

        {reactions.length > 0 && (
          <div className={cn("mt-1 flex gap-1", mine ? "justify-end" : "justify-start")}>
            {reactions.map((r, i) => (
              <button
                key={i}
                onClick={() => r.mine && actions.onReact(m, null)}
                className={cn(
                  "rounded-full px-2 py-0.5 text-[13px]",
                  r.mine ? "bg-brand/20" : "bg-elevated",
                )}
              >
                {r.emoji}
              </button>
            ))}
          </div>
        )}
      </div>
      {!mine && actionBar}
    </div>
  );
}

// ── the chat surface ────────────────────────────────────────────────────────
export function Chat({
  messages,
  onSend,
  actions,
  onEditSubmit,
  onTyping,
  peerTyping,
  pinnedMid,
  onUnpin,
  search,
  placeholder = "Message",
}: {
  messages: ChatMessage[];
  onSend: (payload: OutgoingPayload) => void;
  actions: MsgActions;
  onEditSubmit: (mid: string, text: string) => void;
  onTyping: (on: boolean) => void;
  peerTyping: boolean;
  pinnedMid?: string;
  onUnpin: () => void;
  search: string;
  placeholder?: string;
}) {
  const [text, setText] = useState("");
  const [recSecs, setRecSecs] = useState(0);
  const [recording, setRecording] = useState(false);
  const [replyTo, setReplyTo] = useState<ChatMessage | null>(null);
  const [editing, setEditing] = useState<ChatMessage | null>(null);
  const [jumpMid, setJumpMid] = useState<string | null>(null);
  const [emoji, setEmoji] = useState(false);

  const endRef = useRef<HTMLDivElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const recRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const streamRef = useRef<MediaStream | null>(null);
  const timerRef = useRef<number | undefined>(undefined);
  const secsRef = useRef(0);
  const cancelledRef = useRef(false);
  const typingRef = useRef<number | undefined>(undefined);
  const rowRefs = useRef<Record<string, HTMLDivElement | null>>({});

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length]);

  // Stop announcing "typing" shortly after the last keystroke.
  useEffect(() => () => clearTimeout(typingRef.current), []);
  const keystroke = (v: string) => {
    setText(v);
    onTyping(true);
    clearTimeout(typingRef.current);
    typingRef.current = window.setTimeout(() => onTyping(false), 2500);
  };

  const shown = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return messages;
    return messages.filter((m) => (m.text || summarize(m)).toLowerCase().includes(q));
  }, [messages, search]);

  const pinned = pinnedMid ? messages.find((m) => m.mid === pinnedMid) : undefined;

  const jumpTo = (mid: string) => {
    rowRefs.current[mid]?.scrollIntoView({ behavior: "smooth", block: "center" });
    setJumpMid(mid);
    window.setTimeout(() => setJumpMid(null), 1400);
  };

  const submit = () => {
    const t = text.trim();
    if (!t) return;
    onTyping(false);
    if (editing) {
      onEditSubmit(editing.mid, t);
      setEditing(null);
      setText("");
      return;
    }
    onSend({
      kind: "text",
      text: t,
      ...(replyTo
        ? {
            replyTo: replyTo.mid,
            replyPreview: summarize(replyTo),
            replyAuthor: replyTo.mine ? "You" : "Them",
          }
        : {}),
    });
    setReplyTo(null);
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

  const startEdit = (m: ChatMessage) => {
    setEditing(m);
    setReplyTo(null);
    setText(m.text || "");
  };

  const localActions: MsgActions = {
    ...actions,
    onReply: (m) => {
      setEditing(null);
      setReplyTo(m);
    },
    onEdit: startEdit,
  };

  let lastDay = "";

  return (
    <div className="flex h-full flex-col bg-canvas">
      {pinned && (
        <div className="flex items-center gap-2.5 border-b border-line bg-surface px-4 py-2">
          <PushPin size={15} className="shrink-0 text-brand" weight="fill" />
          <button onClick={() => jumpTo(pinned.mid)} className="min-w-0 flex-1 text-left">
            <p className="text-[11.5px] font-medium text-brand">Pinned message</p>
            <p className="truncate text-[12.5px] text-muted">{summarize(pinned)}</p>
          </button>
          <button
            onClick={onUnpin}
            aria-label="Unpin"
            className="grid h-7 w-7 place-items-center rounded-lg text-muted transition-colors hover:bg-elevated hover:text-ink"
          >
            <X size={14} />
          </button>
        </div>
      )}

      <div className="flex-1 space-y-1.5 overflow-y-auto p-4">
        {shown.length === 0 && (
          <div className="grid h-full place-items-center">
            <p className="text-sm text-muted">
              {search.trim() ? "No messages match that search." : "No messages yet. Say hello."}
            </p>
          </div>
        )}
        {shown.map((m) => {
          const day = dayLabel(m.ts);
          const showDay = day !== lastDay;
          lastDay = day;
          return (
            <div
              key={m.id}
              ref={(el) => {
                rowRefs.current[m.mid] = el;
              }}
            >
              {showDay && (
                <div className="my-3 flex justify-center">
                  <span className="rounded-full bg-black/10 px-2.5 py-0.5 text-[11.5px] text-muted dark:bg-white/10">
                    {day}
                  </span>
                </div>
              )}
              <Bubble
                m={m}
                actions={localActions}
                highlight={jumpMid === m.mid}
                onJumpTo={jumpTo}
              />
            </div>
          );
        })}
        {peerTyping && (
          <p className="px-2 pt-1 text-[12.5px] italic text-muted">typing…</p>
        )}
        <div ref={endRef} />
      </div>

      {(replyTo || editing) && (
        <div className="flex items-center gap-2.5 border-t border-line bg-surface px-4 py-2">
          {editing ? (
            <PencilSimple size={15} className="shrink-0 text-brand" />
          ) : (
            <ArrowBendUpLeft size={15} className="shrink-0 text-brand" />
          )}
          <div className="min-w-0 flex-1">
            <p className="text-[11.5px] font-medium text-brand">
              {editing ? "Edit message" : `Reply to ${replyTo!.mine ? "yourself" : "them"}`}
            </p>
            <p className="truncate text-[12.5px] text-muted">
              {summarize(editing ?? replyTo!)}
            </p>
          </div>
          <button
            onClick={() => {
              setReplyTo(null);
              setEditing(null);
              if (editing) setText("");
            }}
            aria-label="Cancel"
            className="grid h-7 w-7 place-items-center rounded-lg text-muted transition-colors hover:bg-elevated hover:text-ink"
          >
            <X size={14} />
          </button>
        </div>
      )}

      <div className="flex items-center gap-2 border-t border-line bg-surface p-3">
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
            <div className="relative shrink-0">
              <button
                onClick={() => setEmoji((v) => !v)}
                aria-label="Emoji"
                className={cn(
                  "grid h-10 w-10 place-items-center rounded-lg transition-colors hover:bg-elevated",
                  emoji ? "text-brand" : "text-muted hover:text-ink",
                )}
              >
                <Smiley size={21} />
              </button>
              {emoji && (
                <EmojiPicker
                  onPick={(e) => keystroke(text + e)}
                  onClose={() => setEmoji(false)}
                />
              )}
            </div>
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
              onChange={(e) => keystroke(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  submit();
                }
                if (e.key === "Escape") {
                  setEditing(null);
                  setReplyTo(null);
                }
              }}
              placeholder={placeholder}
              className="h-10 flex-1 rounded-lg border border-line bg-canvas px-3.5 text-sm outline-none transition-colors placeholder:text-muted/70 focus:border-brand/50 focus:ring-2 focus:ring-brand/25"
            />
            {text.trim() ? (
              <Button size="icon" onClick={submit} aria-label="Send" className="h-10 w-10 rounded-lg">
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
