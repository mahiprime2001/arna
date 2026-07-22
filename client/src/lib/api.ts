// Same-origin by default: /api and /ws are served from the page's own origin
// (vite proxies them to the Go backend in dev). This means one URL/tunnel covers
// everything and there's no cross-origin or mixed-content trouble. Override with
// VITE_ARNA_API to point at a separate backend origin.
export const API =
  ((import.meta.env as unknown as { VITE_ARNA_API?: string }).VITE_ARNA_API ?? "");

const TOKEN_KEY = "arna_token";

export interface AuthUser {
  id: number;
  email: string;
  name: string;
  handle: string;
  role: string;
}

export const getToken = () => localStorage.getItem(TOKEN_KEY);
export const setToken = (t: string) => localStorage.setItem(TOKEN_KEY, t);
export const clearToken = () => localStorage.removeItem(TOKEN_KEY);

async function req(path: string, opts: RequestInit = {}) {
  const token = getToken();
  const res = await fetch(API + path, {
    ...opts,
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...(opts.headers ?? {}),
    },
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || "Something went wrong. Try again.");
  return data;
}

const asUser = (u: Omit<AuthUser, "role">): AuthUser => ({ ...u, role: "Host" });

export async function signup(email: string, name: string, password: string) {
  const d = await req("/api/signup", {
    method: "POST",
    body: JSON.stringify({ email, name, password }),
  });
  setToken(d.token);
  return asUser(d.user);
}

export async function login(email: string, password: string) {
  const d = await req("/api/login", {
    method: "POST",
    body: JSON.stringify({ email, password }),
  });
  setToken(d.token);
  return asUser(d.user);
}

export async function me() {
  const d = await req("/api/me");
  return asUser(d.user);
}

export async function logout() {
  try {
    await req("/api/logout", { method: "POST" });
  } catch {
    // ignore, we clear locally regardless
  }
  clearToken();
}

// ── Social graph ────────────────────────────────────────────────────────────
import type {
  Friend,
  FriendRequest,
  SearchResult,
  SentRequest,
} from "@/lib/mock";

export async function getFriends(): Promise<{
  friends: Friend[];
  incoming: FriendRequest[];
  outgoing: SentRequest[];
}> {
  return req("/api/friends");
}

export async function sendFriendRequest(handle: string) {
  return req("/api/friends/request", {
    method: "POST",
    body: JSON.stringify({ handle }),
  });
}

export async function respondFriendRequest(id: number, action: "accept" | "decline") {
  return req("/api/friends/respond", {
    method: "POST",
    body: JSON.stringify({ id, action }),
  });
}

export async function cancelFriendRequest(id: number) {
  return req("/api/friends/cancel", { method: "POST", body: JSON.stringify({ id }) });
}

export async function removeFriend(userId: number) {
  return req("/api/friends/remove", {
    method: "POST",
    body: JSON.stringify({ userId }),
  });
}

export async function searchUsers(q: string): Promise<{ users: SearchResult[] }> {
  return req(`/api/users/search?q=${encodeURIComponent(q)}`);
}

export async function ping() {
  return req("/api/presence/ping", { method: "POST" });
}

export async function setPubkey(pubkey: string) {
  return req("/api/keys", { method: "POST", body: JSON.stringify({ pubkey }) });
}
