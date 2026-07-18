import { Plus, StackPlus } from "@phosphor-icons/react";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { PageHeader } from "@/components/PageHeader";
import { workspaces } from "@/lib/mock";

export function Workspaces() {
  return (
    <div className="animate-fade-up space-y-6">
      <PageHeader
        title="Workspaces"
        subtitle="Isolated places you lend to people you invite."
        action={
          <Button>
            <Plus size={16} weight="bold" /> New workspace
          </Button>
        }
      />

      {workspaces.length === 0 && (
        <Card className="flex flex-col items-center gap-3 px-6 py-16 text-center">
          <div className="grid h-14 w-14 place-items-center rounded-2xl bg-brand-soft">
            <StackPlus size={26} weight="duotone" className="text-brand" />
          </div>
          <div className="max-w-sm">
            <h3 className="text-base font-semibold">No workspaces yet</h3>
            <p className="mt-1 text-sm text-muted">
              Create one to lend compute to a friend. They get their own screen,
              and you keep working.
            </p>
          </div>
          <Button className="mt-1">
            <Plus size={16} weight="bold" /> Create workspace
          </Button>
        </Card>
      )}
    </div>
  );
}
