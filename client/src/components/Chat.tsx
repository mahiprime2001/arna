import { useEffect, useRef, useState } from "react";
import { PaperPlaneTilt } from "@phosphor-icons/react";
import { Button } from "@/components/ui/Button";
import { cn } from "@/lib/utils";
import type { ChatMessage } from "@/lib/mock";

// Reusable chat surface: powers direct, room, and workspace chat.
export function Chat({
  messages,
  onSend,
  placeholder = "Message",
}: {
  messages: ChatMessage[];
  onSend: (text: string) => void;
  placeholder?: string;
}) {
  const [text, setText] = useState("");
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length]);

  const send = () => {
    const t = text.trim();
    if (!t) return;
    onSend(t);
    setText("");
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
          <div key={m.id} className={cn("flex", m.mine ? "justify-end" : "justify-start")}>
            <div
              className={cn(
                "max-w-[72%] rounded-2xl px-3.5 py-2 text-sm",
                m.mine
                  ? "rounded-br-md bg-brand text-brand-fg"
                  : "rounded-bl-md bg-elevated text-ink",
              )}
            >
              <p className="whitespace-pre-wrap break-words">{m.text}</p>
              <p
                className={cn(
                  "mt-1 text-[10.5px]",
                  m.mine ? "text-brand-fg/70" : "text-muted",
                )}
              >
                {m.time}
              </p>
            </div>
          </div>
        ))}
        <div ref={endRef} />
      </div>

      <div className="flex items-center gap-2 border-t border-line p-3">
        <input
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send();
            }
          }}
          placeholder={placeholder}
          className="h-10 flex-1 rounded-lg border border-line bg-canvas px-3.5 text-sm outline-none transition-colors placeholder:text-muted/70 focus:border-brand/50 focus:ring-2 focus:ring-brand/25"
        />
        <Button
          size="icon"
          onClick={send}
          disabled={!text.trim()}
          className="h-10 w-10 rounded-lg"
          aria-label="Send"
        >
          <PaperPlaneTilt size={17} weight="fill" />
        </Button>
      </div>
    </div>
  );
}
