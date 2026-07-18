import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { PageHeader } from "@/components/PageHeader";
import { cn } from "@/lib/utils";
import type { Note } from "@/lib/mock";

export function Notifications({
  notes,
  setNotes,
}: {
  notes: Note[];
  setNotes: (n: Note[]) => void;
}) {
  const markAll = () => setNotes(notes.map((n) => ({ ...n, read: true })));
  const markOne = (id: number) =>
    setNotes(notes.map((n) => (n.id === id ? { ...n, read: true } : n)));

  return (
    <div className="animate-fade-up space-y-6">
      <PageHeader
        title="Notifications"
        subtitle="Updates from your workspaces and friends."
        action={
          <Button variant="ghost" onClick={markAll}>
            Mark all read
          </Button>
        }
      />

      <div className="space-y-2.5">
        {notes.map((n) => (
          <Card
            key={n.id}
            onClick={() => markOne(n.id)}
            className={cn(
              "flex cursor-pointer gap-3 p-4 transition-colors hover:bg-elevated",
              !n.read && "border-brand/25",
            )}
          >
            <span
              className={cn(
                "mt-1.5 h-2 w-2 shrink-0 rounded-full",
                n.read ? "bg-transparent" : "bg-brand",
              )}
            />
            <div className="min-w-0 flex-1">
              <div className="flex items-baseline justify-between gap-3">
                <p className="font-medium">{n.title}</p>
                <span className="shrink-0 text-[12px] text-muted">{n.time}</span>
              </div>
              <p className="mt-0.5 text-sm text-muted">{n.body}</p>
            </div>
          </Card>
        ))}
      </div>
    </div>
  );
}
