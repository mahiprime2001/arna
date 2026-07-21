import { useEffect, useState } from "react";
import App from "@/App";
import { Auth } from "@/views/Auth";
import * as api from "@/lib/api";
import type { AuthUser } from "@/lib/api";

// Auth gate: validates any saved session, then shows either login or the app.
export function Root() {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [checking, setChecking] = useState(true);

  useEffect(() => {
    if (!api.getToken()) {
      setChecking(false);
      return;
    }
    api
      .me()
      .then(setUser)
      .catch(() => api.clearToken())
      .finally(() => setChecking(false));
  }, []);

  if (checking) {
    return (
      <div className="grid min-h-screen place-items-center bg-canvas text-sm text-muted">
        Loading
      </div>
    );
  }

  if (!user) return <Auth onAuthed={setUser} />;

  return (
    <App
      user={user}
      onSignOut={() => {
        api.logout();
        setUser(null);
      }}
    />
  );
}
