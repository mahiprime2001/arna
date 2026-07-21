import { useState, type ReactNode } from "react";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import * as api from "@/lib/api";
import type { AuthUser } from "@/lib/api";

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-[13px] font-medium text-muted">{label}</span>
      {children}
    </label>
  );
}

export function Auth({ onAuthed }: { onAuthed: (u: AuthUser) => void }) {
  const [isSignup, setIsSignup] = useState(false);
  const [email, setEmail] = useState("");
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErr("");
    setBusy(true);
    try {
      const u = isSignup
        ? await api.signup(email, name, password)
        : await api.login(email, password);
      onAuthed(u);
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Something went wrong.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="grid min-h-screen place-items-center bg-canvas px-4 text-ink">
      <div className="w-full max-w-sm">
        <div className="mb-6 flex flex-col items-center gap-3 text-center">
          <div className="grid h-12 w-12 place-items-center rounded-xl bg-brand text-xl font-bold text-brand-fg">
            A
          </div>
          <div>
            <h1 className="text-xl font-semibold tracking-tight">
              {isSignup ? "Create your account" : "Welcome back"}
            </h1>
            <p className="mt-1 text-sm text-muted">
              Lend your computer's power, not your computer.
            </p>
          </div>
        </div>

        <div className="rounded-xl border border-line bg-surface p-6 shadow-card">
          <form onSubmit={submit} className="space-y-3.5">
            {isSignup && (
              <Field label="Name">
                <Input
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Your name"
                  autoComplete="name"
                />
              </Field>
            )}
            <Field label="Email">
              <Input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
                autoComplete="email"
              />
            </Field>
            <Field label="Password">
              <Input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="6+ characters"
                autoComplete={isSignup ? "new-password" : "current-password"}
              />
            </Field>
            {err && <p className="text-[13px] text-danger">{err}</p>}
            <Button type="submit" disabled={busy} className="w-full">
              {busy ? "Please wait" : isSignup ? "Create account" : "Sign in"}
            </Button>
          </form>
        </div>

        <p className="mt-4 text-center text-sm text-muted">
          {isSignup ? "Already have an account?" : "New to Arna?"}{" "}
          <button
            onClick={() => {
              setIsSignup((v) => !v);
              setErr("");
            }}
            className="font-medium text-brand hover:underline"
          >
            {isSignup ? "Sign in" : "Create one"}
          </button>
        </p>
      </div>
    </div>
  );
}
