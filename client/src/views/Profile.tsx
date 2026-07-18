import { PencilSimple, SignOut } from "@phosphor-icons/react";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Avatar } from "@/components/Avatar";
import { PageHeader } from "@/components/PageHeader";
import { user } from "@/lib/mock";

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between px-5 py-3.5">
      <span className="text-sm text-muted">{label}</span>
      <span className="text-sm font-medium">{value}</span>
    </div>
  );
}

export function Profile() {
  return (
    <div className="animate-fade-up space-y-6">
      <PageHeader title="Profile" />

      <Card className="flex items-center gap-4 p-6">
        <Avatar name={user.name} size={64} />
        <div>
          <h2 className="text-xl font-semibold tracking-tight">{user.name}</h2>
          <p className="text-sm text-muted">{user.email}</p>
          <span className="mt-2 inline-flex rounded-full bg-brand-soft px-2.5 py-0.5 text-[12px] font-medium text-brand">
            {user.role}
          </span>
        </div>
      </Card>

      <Card className="divide-y divide-line">
        <Row label="Display name" value={user.name} />
        <Row label="Email" value={user.email} />
        <Row label="Role" value={user.role} />
      </Card>

      <div className="flex gap-3">
        <Button variant="outline">
          <PencilSimple size={16} /> Edit profile
        </Button>
        <Button variant="danger">
          <SignOut size={16} /> Sign out
        </Button>
      </div>
    </div>
  );
}
