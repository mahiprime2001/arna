import { useEffect, useMemo, useState } from "react";
import { TitleBar } from "@/components/TitleBar";
import { Sidebar } from "@/components/Sidebar";
import { Dashboard } from "@/views/Dashboard";
import { Workspaces } from "@/views/Workspaces";
import { Notifications } from "@/views/Notifications";
import { Profile } from "@/views/Profile";
import { Settings } from "@/views/Settings";
import { initialNotes, user, type Note, type Route } from "@/lib/mock";

export type Theme = "dark" | "light";

export default function App() {
  const [route, setRoute] = useState<Route>("dashboard");
  const [theme, setTheme] = useState<Theme>("dark");
  const [notes, setNotes] = useState<Note[]>(initialNotes);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", theme === "dark");
  }, [theme]);

  const unread = useMemo(() => notes.filter((n) => !n.read).length, [notes]);

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-canvas text-ink">
      <TitleBar unread={unread} onBell={() => setRoute("notifications")} />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar route={route} setRoute={setRoute} unread={unread} user={user} />
        <main className="flex-1 overflow-y-auto">
          <div className="mx-auto max-w-5xl px-8 py-8">
            {route === "dashboard" && (
              <Dashboard unread={unread} setRoute={setRoute} />
            )}
            {route === "workspaces" && <Workspaces />}
            {route === "notifications" && (
              <Notifications notes={notes} setNotes={setNotes} />
            )}
            {route === "profile" && <Profile />}
            {route === "settings" && <Settings theme={theme} setTheme={setTheme} />}
          </div>
        </main>
      </div>
    </div>
  );
}
