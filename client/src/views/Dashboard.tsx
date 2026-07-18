import {
  StackSimple,
  UsersThree,
  BellRinging,
  Plus,
} from "@phosphor-icons/react";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Avatar } from "@/components/Avatar";
import { PageHeader } from "@/components/PageHeader";
import { cn } from "@/lib/utils";
import { friends, user, workspaces, type Route } from "@/lib/mock";

export function Dashboard({
  unread,
  setRoute,
}: {
  unread: number;
  setRoute: (r: Route) => void;
}) {
  const online = friends.filter((f) => f.online).length;
  const first = user.name.split(" ")[0];
  const stats = [
    { icon: StackSimple, value: workspaces.length, label: "Active workspaces" },
    { icon: UsersThree, value: online, label: "Friends online" },
    { icon: BellRinging, value: unread, label: "Unread alerts" },
  ];

  return (
    <div className="animate-fade-up space-y-6">
      <PageHeader
        title={`Welcome back, ${first}`}
        subtitle="Here is what is happening on your machine."
      />

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

        <Card className="p-5">
          <p className="text-[13px] font-medium text-muted">Friends</p>
          <ul className="mt-3 space-y-3">
            {friends.map((f) => (
              <li key={f.name} className="flex items-center gap-3">
                <Avatar name={f.name} size={30} />
                <span className="text-sm">{f.name}</span>
                <span
                  className={cn(
                    "ml-auto h-2 w-2 rounded-full",
                    f.online ? "bg-good" : "bg-line",
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
