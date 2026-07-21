import {
  StackSimple,
  UsersThree,
  BellRinging,
  Plus,
  ArrowRight,
} from "@phosphor-icons/react";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Avatar } from "@/components/Avatar";
import { PageHeader } from "@/components/PageHeader";
import { cn } from "@/lib/utils";
import { workspaces, type Friend, type Route } from "@/lib/mock";
import type { AuthUser } from "@/lib/api";

export function Dashboard({
  user,
  friends,
  requestCount,
  unread,
  setRoute,
}: {
  user: AuthUser;
  friends: Friend[];
  requestCount: number;
  unread: number;
  setRoute: (r: Route) => void;
}) {
  const online = friends.filter((f) => f.presence !== "offline");
  const first = user.name.split(" ")[0];
  const stats = [
    { icon: StackSimple, value: workspaces.length, label: "Active workspaces" },
    { icon: UsersThree, value: online.length, label: "Friends online" },
    { icon: BellRinging, value: unread, label: "Unread alerts" },
  ];

  return (
    <div className="animate-fade-up space-y-6">
      <PageHeader
        title={`Welcome back, ${first}`}
        subtitle="A quick look at your machine and your people."
      />

      {requestCount > 0 && (
        <button
          onClick={() => setRoute("friends")}
          className="flex w-full items-center gap-3 rounded-lg border border-brand/25 bg-brand-soft px-4 py-3 text-left transition-colors hover:bg-brand/15"
        >
          <UsersThree size={20} weight="duotone" className="text-brand" />
          <p className="text-sm">
            <span className="font-medium">
              {requestCount} friend {requestCount === 1 ? "request" : "requests"}
            </span>{" "}
            <span className="text-muted">waiting for you.</span>
          </p>
          <span className="ml-auto inline-flex items-center gap-1 text-[13px] font-medium text-brand">
            Review <ArrowRight size={14} weight="bold" />
          </span>
        </button>
      )}

      <div className="grid grid-cols-3 gap-4">
        {stats.map(({ icon: Ico, value, label }) => (
          <Card key={label} className="p-5">
            <Ico size={20} weight="duotone" className="text-brand" />
            <p className="mt-3 text-3xl font-semibold tracking-tight">{value}</p>
            <p className="text-[13px] text-muted">{label}</p>
          </Card>
        ))}
      </div>

      <div className="grid grid-cols-3 gap-4">
        <Card className="col-span-2 flex items-center justify-between gap-6 p-6">
          <div>
            <h3 className="text-base font-semibold">Start a workspace</h3>
            <p className="mt-1 max-w-md text-sm text-muted">
              Lend some compute to a friend. They get their own space, you keep
              your desktop.
            </p>
          </div>
          <Button onClick={() => setRoute("workspaces")}>
            <Plus size={16} weight="bold" /> New workspace
          </Button>
        </Card>

        <Card className="flex flex-col p-5">
          <div className="flex items-center justify-between">
            <p className="text-[13px] font-medium text-muted">Friends</p>
            <button
              onClick={() => setRoute("friends")}
              className="text-[12px] font-medium text-brand hover:underline"
            >
              See all
            </button>
          </div>
          <ul className="mt-3 space-y-3">
            {friends.slice(0, 4).map((f) => (
              <li key={f.id} className="flex items-center gap-3">
                <Avatar name={f.name} size={30} />
                <span className="truncate text-sm">{f.name}</span>
                <span
                  className={cn(
                    "ml-auto h-2 w-2 shrink-0 rounded-full",
                    f.presence === "online"
                      ? "bg-good"
                      : f.presence === "workspace"
                        ? "bg-warn"
                        : "bg-line",
                  )}
                />
              </li>
            ))}
          </ul>
        </Card>
      </div>

      <div>
        <h3 className="mb-3 text-sm font-semibold text-muted">Recent activity</h3>
        <Card className="grid place-items-center gap-1 p-10 text-center">
          <p className="text-sm">Nothing here yet.</p>
          <p className="text-[13px] text-muted">
            Your workspace activity will show up here.
          </p>
        </Card>
      </div>
    </div>
  );
}
