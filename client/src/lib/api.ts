// Talks to the Go services backend. The base URL is derived from the page host,
// so opening the app from another device on the LAN just works (that device's
// URL points the API at the same host).
export const API = `http://${location.hostname}:8787`;

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
