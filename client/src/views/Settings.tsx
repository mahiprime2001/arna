import { useState } from "react";
import { Moon, Sun } from "@phosphor-icons/react";
import { Card } from "@/components/ui/Card";
import { Switch } from "@/components/ui/Switch";
import { PageHeader } from "@/components/PageHeader";
import { cn } from "@/lib/utils";
import type { Theme } from "@/App";

function ToggleRow({
  label,
  desc,
  value,
  onChange,
}: {
  label: string;
  desc: string;
  value: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-6 px-5 py-4">
      <div>
        <p className="text-sm font-medium">{label}</p>
        <p className="text-[13px] text-muted">{desc}</p>
      </div>
      <Switch checked={value} onCheckedChange={onChange} />
    </div>
  );
}

function ThemeChip({
  active,
  onClick,
  icon: Ico,
  label,
}: {
  active: boolean;
  onClick: () => void;
  icon: typeof Moon;
  label: string;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-2 rounded-md border px-3.5 py-2 text-sm font-medium transition-colors",
        active
          ? "border-brand/40 bg-brand-soft text-brand"
          : "border-line text-muted hover:bg-elevated hover:text-ink",
      )}
    >
      <Ico size={16} weight={active ? "fill" : "regular"} /> {label}
    </button>
  );
}

export function Settings({
  theme,
  setTheme,
}: {
  theme: Theme;
  setTheme: (t: Theme) => void;
}) {
  const [launch, setLaunch] = useState(true);
  const [motion, setMotion] = useState(false);
  const [offline, setOffline] = useState(true);

  return (
    <div className="animate-fade-up space-y-8">
      <PageHeader
        title="Settings"
        subtitle="Preferences for this device. Mock only for now."
      />

      <section className="space-y-3">
        <h2 className="text-sm font-semibold text-muted">Appearance</h2>
        <Card className="flex items-center justify-between gap-6 p-5">
          <div>
            <p className="text-sm font-medium">Theme</p>
            <p className="text-[13px] text-muted">Switch between dark and light.</p>
          </div>
          <div className="flex gap-2">
            <ThemeChip
              active={theme === "dark"}
              onClick={() => setTheme("dark")}
              icon={Moon}
              label="Dark"
            />
            <ThemeChip
              active={theme === "light"}
              onClick={() => setTheme("light")}
              icon={Sun}
              label="Light"
            />
          </div>
        </Card>
      </section>

      <section className="space-y-3">
        <h2 className="text-sm font-semibold text-muted">General</h2>
        <Card className="divide-y divide-line">
          <ToggleRow
            label="Launch on startup"
            desc="Open Arna when this computer starts."
            value={launch}
            onChange={setLaunch}
          />
          <ToggleRow
            label="Reduce motion"
            desc="Minimize animations across the app."
            value={motion}
            onChange={setMotion}
          />
          <ToggleRow
            label="Show offline friends"
            desc="List friends who are not online right now."
            value={offline}
            onChange={setOffline}
          />
        </Card>
      </section>

      <section className="space-y-3">
        <h2 className="text-sm font-semibold text-muted">About</h2>
        <Card className="divide-y divide-line">
          <div className="flex items-center justify-between px-5 py-3.5">
            <span className="text-sm text-muted">Version</span>
            <span className="font-mono text-[13px]">0.1.0 shell</span>
          </div>
          <div className="flex items-center justify-between px-5 py-3.5">
            <span className="text-sm text-muted">Build</span>
            <span className="font-mono text-[13px]">mock data only</span>
          </div>
        </Card>
      </section>
    </div>
  );
}
