import { computed, ref } from "vue";

export type Device = { id: string; name: string; has_password?: boolean };
export type DeviceInfo = { id: string; name: string; has_password: boolean };

/**
 * Account session: sign up / log in against the backend's HTTP API, remember the
 * session token, and list/register the user's devices. The backend's HTTP base is
 * derived from the WebSocket URL the console already uses (ws→http, wss→https).
 */
export function useAuth() {
  const token = ref<string | null>(localStorage.getItem("arna.session"));
  const email = ref<string | null>(localStorage.getItem("arna.email"));
  const userId = ref<string | null>(localStorage.getItem("arna.userId"));
  const devices = ref<Device[]>([]);
  const authError = ref("");
  const busy = ref(false);
  const loggedIn = computed(() => !!token.value);

  function httpBase(wsUrl: string): string {
    try {
      const u = new URL(wsUrl);
      u.protocol = u.protocol === "wss:" ? "https:" : "http:";
      return u.origin;
    } catch {
      return "";
    }
  }

  function setSession(t: string, em: string, uid?: string) {
    token.value = t;
    email.value = em;
    localStorage.setItem("arna.session", t);
    localStorage.setItem("arna.email", em);
    if (uid) {
      userId.value = uid;
      localStorage.setItem("arna.userId", uid);
    }
  }

  function logout() {
    token.value = null;
    email.value = null;
    userId.value = null;
    devices.value = [];
    localStorage.removeItem("arna.session");
    localStorage.removeItem("arna.email");
    localStorage.removeItem("arna.userId");
  }

  async function authPost(wsUrl: string, path: string, body: object) {
    const r = await fetch(httpBase(wsUrl) + path, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    return { status: r.status, json: await r.json().catch(() => null), text: () => r.text() };
  }

  async function authenticate(kind: "login" | "signup", wsUrl: string, em: string, pw: string) {
    authError.value = "";
    busy.value = true;
    try {
      const { status, json } = await authPost(wsUrl, `/auth/${kind}`, { email: em, password: pw });
      if (status === 200 && json?.token) {
        setSession(json.token, em, json.user_id);
        await refreshDevices(wsUrl);
        return true;
      }
      if (status === 401) authError.value = "Wrong email or password.";
      else if (status === 409) authError.value = "That email is already registered — try logging in.";
      else if (status === 400) authError.value = "Enter a valid email and a password of at least 8 characters.";
      else if (status === 503) authError.value = "Accounts aren't enabled on this server.";
      else authError.value = "Something went wrong. Try again.";
      return false;
    } catch {
      authError.value = "Can't reach the server. Check the address.";
      return false;
    } finally {
      busy.value = false;
    }
  }

  const login = (wsUrl: string, em: string, pw: string) => authenticate("login", wsUrl, em, pw);
  const signup = (wsUrl: string, em: string, pw: string) => authenticate("signup", wsUrl, em, pw);

  async function refreshDevices(wsUrl: string) {
    if (!token.value) return;
    try {
      const r = await fetch(httpBase(wsUrl) + "/devices", {
        headers: { authorization: "Bearer " + token.value },
      });
      if (r.status === 401) {
        logout();
        return;
      }
      if (r.ok) devices.value = await r.json();
    } catch {
      /* leave the list as-is */
    }
  }

  /** Register a device under this account; returns its agent token (to paste into the agent). */
  async function addDevice(wsUrl: string, id: string, name: string): Promise<string | null> {
    if (!token.value) return null;
    authError.value = "";
    try {
      const r = await fetch(httpBase(wsUrl) + "/devices", {
        method: "POST",
        headers: { "content-type": "application/json", authorization: "Bearer " + token.value },
        body: JSON.stringify({ id, name }),
      });
      if (!r.ok) {
        authError.value = "Couldn't add the device.";
        return null;
      }
      const json = await r.json();
      await refreshDevices(wsUrl);
      return json.token as string;
    } catch {
      authError.value = "Can't reach the server.";
      return null;
    }
  }

  /** Refresh the short account ID + email from the server (e.g. on load). */
  async function fetchMe(wsUrl: string) {
    if (!token.value) return;
    try {
      const r = await fetch(httpBase(wsUrl) + "/me", {
        headers: { authorization: "Bearer " + token.value },
      });
      if (r.status === 401) {
        logout();
        return;
      }
      if (r.ok) {
        const j = await r.json();
        if (j.user_id) {
          userId.value = j.user_id;
          localStorage.setItem("arna.userId", j.user_id);
        }
        if (j.email) {
          email.value = j.email;
          localStorage.setItem("arna.email", j.email);
        }
      }
    } catch {
      /* offline — keep cached values */
    }
  }

  /** Set (pw) or clear (null/"") a device's unattended-access password. */
  async function setDevicePassword(wsUrl: string, id: string, pw: string | null): Promise<boolean> {
    if (!token.value) return false;
    try {
      const r = await fetch(httpBase(wsUrl) + `/devices/${encodeURIComponent(id)}/password`, {
        method: "POST",
        headers: { "content-type": "application/json", authorization: "Bearer " + token.value },
        body: JSON.stringify({ password: pw || null }),
      });
      if (r.ok) await refreshDevices(wsUrl);
      return r.ok;
    } catch {
      return false;
    }
  }

  /** Resolve a device's name + whether it takes a password, before connecting. */
  async function lookupDevice(wsUrl: string, id: string): Promise<DeviceInfo | null> {
    try {
      const r = await fetch(httpBase(wsUrl) + `/device/${encodeURIComponent(id)}`);
      if (!r.ok) return null;
      return (await r.json()) as DeviceInfo;
    } catch {
      return null;
    }
  }

  return {
    token,
    email,
    userId,
    devices,
    loggedIn,
    authError,
    busy,
    login,
    signup,
    logout,
    refreshDevices,
    addDevice,
    fetchMe,
    setDevicePassword,
    lookupDevice,
  };
}
