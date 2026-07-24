import { useState } from "react";
import {
  X,
  Clock,
  FloppyDisk,
  Folder,
  Plus,
  Trash,
  Info,
  ShieldCheck,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { Switch } from "@/components/ui/Switch";
import { cn } from "@/lib/utils";
import {
  CATALOG,
  CAPABILITY_LABEL,
  DEFAULT_CAPABILITIES,
  draftWorkspace,
  type Capability,
  type Workspace,
} from "@/lib/workspace";

function Section({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="border-t border-line px-6 py-5 first:border-t-0">
      <h3 className="text-sm font-semibold">{title}</h3>
      {hint && <p className="mt-0.5 text-[12.5px] leading-relaxed text-muted">{hint}</p>}
      <div className="mt-3.5">{children}</div>
    </section>
  );
}

function Choice({
  active,
  onClick,
  icon,
  title,
  hint,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  title: string;
  hint: string;
}) {
  return (
    <button
      onClick={onClick}
      aria-pressed={active}
      className={cn(
        "flex flex-1 gap-3 rounded-lg border p-3 text-left transition-colors",
        active ? "border-brand bg-brand-soft" : "border-line hover:bg-elevated",
      )}
    >
      <span className={cn("mt-0.5 shrink-0", active ? "text-brand" : "text-muted")}>{icon}</span>
      <span className="min-w-0">
        <span className="block text-[13.5px] font-medium">{title}</span>
        <span className="mt-0.5 block text-[12px] leading-relaxed text-muted">{hint}</span>
      </span>
    </button>
  );
}

function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4 py-2">
      <div className="min-w-0">
        <p className="text-[13.5px]">{label}</p>
        {hint && <p className="text-[12px] text-muted">{hint}</p>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

export function CreateWorkspace({
  onClose,
  onCreate,
}: {
  onClose: () => void;
  onCreate: (draft: Omit<Workspace, "id" | "state" | "createdAt">) => void;
}) {
  const [d, setD] = useState(draftWorkspace());
  const [sharePath, setSharePath] = useState("");
  const set = <K extends keyof typeof d>(k: K, v: (typeof d)[K]) =>
    setD((prev) => ({ ...prev, [k]: v }));

  const toggleApp = (id: string) =>
    set("apps", d.apps.includes(id) ? d.apps.filter((a) => a !== id) : [...d.apps, id]);

  const addShare = () => {
    const p = sharePath.trim();
    if (!p) return;
    set("shares", [
      ...d.shares,
      { id: crypto.randomUUID(), path: p, access: "ro" as const },
    ]);
    setSharePath("");
  };

  // Only capabilities the spec marks "configurable" are the owner's to decide.
  const configurable = (Object.keys(CAPABILITY_LABEL) as Capability[]).filter(
    (c) => DEFAULT_CAPABILITIES.collaborator[c] === "configurable",
  );

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center overflow-y-auto bg-black/50 p-6"
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="my-auto w-full max-w-lg overflow-hidden rounded-xl border border-line bg-surface shadow-pop"
      >
        <header className="flex items-center gap-3 border-b border-line px-6 py-4">
          <div>
            <h2 className="text-base font-semibold">New workspace</h2>
            <p className="text-[12.5px] text-muted">
              An isolated place on this machine. It can't see your screen, files, or
              other apps.
            </p>
          </div>
          <button
            onClick={onClose}
            aria-label="Close"
            className="ml-auto grid h-8 w-8 shrink-0 place-items-center rounded-lg text-muted transition-colors hover:bg-elevated hover:text-ink"
          >
            <X size={16} />
          </button>
        </header>

        <div className="max-h-[65vh] overflow-y-auto">
          <Section title="Name">
            <Input
              autoFocus
              value={d.name}
              onChange={(e) => set("name", e.target.value)}
              placeholder="Design review, Pair session, Client sandbox…"
            />
          </Section>

          <Section
            title="What happens when it closes"
            hint="Both are proper choices — one isn't a lesser version of the other."
          >
            <div className="flex gap-2.5">
              <Choice
                active={d.persistence === "saved"}
                onClick={() => set("persistence", "saved")}
                icon={<FloppyDisk size={18} />}
                title="Keep it"
                hint="Files and installed tools survive. Pick up where you left off."
              />
              <Choice
                active={d.persistence === "temporary"}
                onClick={() => set("persistence", "temporary")}
                icon={<Clock size={18} />}
                title="Wipe it"
                hint="Everything inside is destroyed for good when it closes."
              />
            </div>
          </Section>

          <Section title="Apps" hint="Only what you pick can run inside. You can change this later.">
            <div className="grid grid-cols-2 gap-2">
              {CATALOG.map((a) => (
                <button
                  key={a.id}
                  onClick={() => toggleApp(a.id)}
                  aria-pressed={d.apps.includes(a.id)}
                  className={cn(
                    "rounded-lg border p-2.5 text-left transition-colors",
                    d.apps.includes(a.id)
                      ? "border-brand bg-brand-soft"
                      : "border-line hover:bg-elevated",
                  )}
                >
                  <p className="text-[13.5px] font-medium">{a.name}</p>
                  <p className="text-[12px] text-muted">{a.hint}</p>
                </button>
              ))}
            </div>
          </Section>

          <Section
            title="Shared folders"
            hint="Nothing on your machine is reachable unless you add it here. Folders you don't share aren't hidden — they don't exist as far as the workspace is concerned."
          >
            <div className="flex gap-2">
              <Input
                value={sharePath}
                onChange={(e) => setSharePath(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), addShare())}
                placeholder="D:\projects\client-site"
              />
              <Button variant="outline" size="icon" onClick={addShare} aria-label="Add folder">
                <Plus size={16} />
              </Button>
            </div>
            {d.shares.length > 0 && (
              <ul className="mt-2.5 space-y-1.5">
                {d.shares.map((s) => (
                  <li key={s.id} className="flex items-center gap-2.5 rounded-lg bg-elevated px-2.5 py-2">
                    <Folder size={15} className="shrink-0 text-muted" />
                    <span className="min-w-0 flex-1 truncate font-mono text-[12px]">{s.path}</span>
                    <button
                      onClick={() =>
                        set(
                          "shares",
                          d.shares.map((x) =>
                            x.id === s.id ? { ...x, access: x.access === "ro" ? "rw" : "ro" } : x,
                          ),
                        )
                      }
                      className="shrink-0 rounded-md bg-surface px-2 py-0.5 text-[11.5px] font-medium transition-colors hover:text-brand"
                    >
                      {s.access === "ro" ? "Read only" : "Can edit"}
                    </button>
                    <button
                      onClick={() => set("shares", d.shares.filter((x) => x.id !== s.id))}
                      aria-label={`Remove ${s.path}`}
                      className="shrink-0 text-muted transition-colors hover:text-danger"
                    >
                      <Trash size={14} />
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </Section>

          <Section
            title="What guests can do"
            hint="Applies to people you invite as collaborators. Observers can only watch — they can never type or copy anything out."
          >
            {configurable.map((c) => (
              <Row key={c} label={CAPABILITY_LABEL[c]}>
                <Switch
                  checked={d.collaboratorGrants[c]}
                  onCheckedChange={(v) =>
                    set("collaboratorGrants", { ...d.collaboratorGrants, [c]: v })
                  }
                />
              </Row>
            ))}
          </Section>

          <Section title="Limits" hint="Leave blank for no limit.">
            <div className="grid grid-cols-3 gap-2">
              {(
                [
                  ["cpuCores", "CPU cores"],
                  ["memoryGb", "Memory (GB)"],
                  ["storageGb", "Disk (GB)"],
                ] as const
              ).map(([k, label]) => (
                <label key={k} className="block">
                  <span className="mb-1 block text-[12px] text-muted">{label}</span>
                  <Input
                    type="number"
                    min={1}
                    value={d.limits[k] ?? ""}
                    onChange={(e) =>
                      set("limits", {
                        ...d.limits,
                        [k]: e.target.value ? Number(e.target.value) : undefined,
                      })
                    }
                    placeholder="—"
                  />
                </label>
              ))}
            </div>
          </Section>

          <Section title="Network and power">
            <Row label="Internet access" hint="Your local network is always blocked.">
              <Switch checked={d.internet} onCheckedChange={(v) => set("internet", v)} />
            </Row>
            <Row
              label="Keep running when nobody's connected"
              hint={d.whenEmpty === "pause" ? "Pauses when everyone leaves" : "Stays running"}
            >
              <Switch
                checked={d.whenEmpty === "keep-running"}
                onCheckedChange={(v) => set("whenEmpty", v ? "keep-running" : "pause")}
              />
            </Row>
          </Section>

          <div className="mx-6 mb-5 flex gap-2.5 rounded-lg bg-elevated p-3">
            <ShieldCheck size={17} className="mt-0.5 shrink-0 text-good" />
            <p className="text-[12px] leading-relaxed text-muted">
              Whatever you pick, the workspace can never see your screen, your keyboard,
              your clipboard, your other apps, or any folder you didn't share. That part
              isn't a setting.
            </p>
          </div>
        </div>

        <footer className="flex items-center gap-3 border-t border-line px-6 py-4">
          <p className="flex items-center gap-1.5 text-[12px] text-muted">
            <Info size={14} />
            Won't start yet — needs the platform layer
          </p>
          <div className="ml-auto flex gap-2">
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button disabled={!d.name.trim() || d.apps.length === 0} onClick={() => onCreate(d)}>
              Create
            </Button>
          </div>
        </footer>
      </div>
    </div>
  );
}
