import { House, StackSimple, Bell, UserCircle, GearSix } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { Avatar } from "./Avatar";
import { cn } from "@/lib/utils";
import type { Route, User } from "@/lib/mock";

const ITEMS: { id: Route; label: string; icon: Icon }[] = [
  { id: "dashboard", label: "Dashboard", icon: House },
  { id: "workspaces", label: "Workspaces", icon: StackSimple },
  { id: "notifications", label: "Notifications", icon: Bell },
  { id: "profile", label: "Profile", icon: UserCircle },
  { id: "settings", label: "Settings", icon: GearSix },
];

export function Sidebar({
  route,
  setRoute,
  unread,
  user,
}: {
  route: Route;
  setRoute: (r: Route) => void;
  unread: number;
  user: User;
}) {
  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-line bg-surface">
      <nav className="flex-1 space-y-0.5 p-3">
        <p className="px-3 pb-2 pt-1 text-[11px] font-semibold uppercase tracking-wider text-muted">
          Menu
        </p>
        {ITEMS.map(({ id, label, icon: Ico }) => {
          const active = route === id;
          const badge = id === "notifications" ? unread : 0;
          return (
            <button
              key={id}
              onClick={() => setRoute(id)}
              className={cn(
                "group relative flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
                active
                  ? "bg-brand-soft text-ink"
                  : "text-muted hover:bg-elevated hover:text-ink",
              )}
            >
              {active && (
                <span className="absolute left-0 top-1/2 h-4 w-[3px] -translate-y-1/2 rounded-r-full bg-brand" />
              )}
              <Ico
                size={18}
                weight={active ? "fill" : "regular"}
                className={active ? "text-brand" : ""}
              />
              <span className="font-medium">{label}</span>
              {badge > 0 && (
                <span className="ml-auto grid h-5 min-w-[20px] place-items-center rounded-full bg-brand px-1.5 text-[11px] font-semibold text-brand-fg">
                  {badge}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      <button
        onClick={() => setRoute("profile")}
        className="m-3 flex items-center gap-3 rounded-lg border border-line p-2.5 text-left transition-colors hover:bg-elevated"
      >
        <Avatar name={user.name} size={34} />
        <div className="min-w-0">
          <p className="truncate text-[13px] font-medium">{user.name}</p>
          <p className="truncate text-[12px] text-muted">{user.role}</p>
        </div>
      </button>
    </aside>
  );
}
