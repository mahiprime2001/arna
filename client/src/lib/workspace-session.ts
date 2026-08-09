// Watch & Control transport. A host shares a workspace SURFACE (video) with a
// guest and receives the guest's input over a data channel — which it hands to
// the backend gate (RemoteSession::input), so a Viewer's input is rejected there,
// not here. Capture is abstracted behind a SurfaceProvider so WebRTC doesn't care
// whether the pixels came from a Docker window or a native desktop. See
// docs/watch-and-control.md.
import { invoke } from "@tauri-apps/api/core";

// ── input protocol (must match remote.rs `InputEvent`, serde tag "kind") ──────
export type InputEvent =
  | { kind: "move"; x: number; y: number } // normalised 0..1 over the surface
  | { kind: "button"; button: "left" | "right" | "middle"; down: boolean }
  | { kind: "key"; vk: number; down: boolean }
  | { kind: "scroll"; dy: number };

// ── SurfaceProvider: where the pixels come from (WebRTC is agnostic) ──────────
export interface SurfaceProvider {
  label: string;
  capture(): Promise<MediaStream>;
}

/** Docker (and any windowed) surface: the host picks the workspace window from
 *  the browser's display-capture picker. The guest never gets the workspace URL. */
export const DockerSurface: SurfaceProvider = {
  label: "workspace window",
  async capture() {
    const md = navigator.mediaDevices as MediaDevices & {
      getDisplayMedia?: (c: DisplayMediaStreamOptions) => Promise<MediaStream>;
    };
    if (!md?.getDisplayMedia) {
      throw new Error("Screen capture isn't available here — open the desktop app.");
    }
    return md.getDisplayMedia({ video: true, audio: false });
  },
};

/** Native Windows surface — capture the workspace desktop. A later slice; the
 *  session/transport/gate above it are identical. */
export const NativeWindowsSurface: SurfaceProvider = {
  label: "native desktop",
  async capture(): Promise<MediaStream> {
    throw new Error("Native surface capture isn't implemented yet.");
  },
};

const ICE: RTCIceServer[] = [
  { urls: "stun:stun.l.google.com:19302" },
  { urls: "stun:stun1.l.google.com:19302" },
];

export type SessionSignal = {
  ns: "wss";
  workspace: string;
  t: "offer" | "answer" | "ice" | "end";
  sdp?: RTCSessionDescriptionInit;
  candidate?: RTCIceCandidateInit;
};

export type Signaler = (to: number, signal: SessionSignal) => void;

// ── host: share a surface, forward guest input to the backend gate ───────────
export class HostSession {
  private pc: RTCPeerConnection | null = null;
  private stream: MediaStream | null = null;

  constructor(
    private workspace: string,
    private guestUid: number,
    /** the session guest id the gate knows (from the invite/join) */
    private guestId: string,
    private send: Signaler,
  ) {}

  async share(surface: SurfaceProvider): Promise<void> {
    this.stream = await surface.capture();
    const pc = new RTCPeerConnection({ iceServers: ICE });
    this.pc = pc;
    this.stream.getTracks().forEach((t) => pc.addTrack(t, this.stream!));

    // The host CREATES the input channel; guest input arrives here and is handed
    // straight to the gate — never trusted, never injected without the gate's ok.
    const input = pc.createDataChannel("input");
    input.onmessage = (e) => this.gateInput(String(e.data));

    pc.onicecandidate = (e) => {
      if (e.candidate) this.emit("ice", { candidate: e.candidate.toJSON() });
    };
    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    this.emit("offer", { sdp: offer });
  }

  async onSignal(sig: SessionSignal): Promise<void> {
    if (!this.pc) return;
    if (sig.t === "answer" && sig.sdp) {
      await this.pc.setRemoteDescription(sig.sdp);
    } else if (sig.t === "ice" && sig.candidate) {
      try {
        await this.pc.addIceCandidate(sig.candidate);
      } catch {
        /* stale candidate */
      }
    }
  }

  private async gateInput(event: string): Promise<void> {
    // Every event is offered to the backend gate; Viewer input returns false and
    // is dropped. Enforcement lives there, not here.
    try {
      await invoke("remote_input", { workspace: this.workspace, guest: this.guestId, event });
    } catch {
      /* backend unavailable */
    }
  }

  private emit(t: SessionSignal["t"], extra: Partial<SessionSignal>) {
    this.send(this.guestUid, { ns: "wss", workspace: this.workspace, t, ...extra });
  }

  stop() {
    this.stream?.getTracks().forEach((t) => t.stop());
    try {
      this.pc?.close();
    } catch {
      /* ignore */
    }
    this.pc = null;
    this.stream = null;
  }
}

// ── guest: receive the surface, capture + send input ─────────────────────────
export class GuestSession {
  private pc: RTCPeerConnection | null = null;
  private input: RTCDataChannel | null = null;

  constructor(
    private workspace: string,
    private hostUid: number,
    private send: Signaler,
    private onStream: (s: MediaStream) => void,
  ) {}

  async onSignal(sig: SessionSignal): Promise<void> {
    if (sig.t === "offer" && sig.sdp) {
      const pc = new RTCPeerConnection({ iceServers: ICE });
      this.pc = pc;
      pc.ontrack = (e) => this.onStream(e.streams[0] ?? new MediaStream([e.track]));
      pc.ondatachannel = (e) => {
        this.input = e.channel;
      };
      pc.onicecandidate = (e) => {
        if (e.candidate) this.emit("ice", { candidate: e.candidate.toJSON() });
      };
      await pc.setRemoteDescription(sig.sdp);
      const answer = await pc.createAnswer();
      await pc.setLocalDescription(answer);
      this.emit("answer", { sdp: answer });
    } else if (sig.t === "ice" && sig.candidate && this.pc) {
      try {
        await this.pc.addIceCandidate(sig.candidate);
      } catch {
        /* ignore */
      }
    }
  }

  /** Send an input event. It may be gated (dropped) on the host — that's correct;
   *  the guest always sends, the host always decides. */
  sendInput(ev: InputEvent) {
    if (this.input && this.input.readyState === "open") {
      this.input.send(JSON.stringify(ev));
    }
  }

  private emit(t: SessionSignal["t"], extra: Partial<SessionSignal>) {
    this.send(this.hostUid, { ns: "wss", workspace: this.workspace, t, ...extra });
  }

  stop() {
    try {
      this.pc?.close();
    } catch {
      /* ignore */
    }
    this.pc = null;
    this.input = null;
  }
}

// ── guest capture: translate DOM events over the video into InputEvents ──────
const MOUSE_BUTTON: Record<number, "left" | "right" | "middle"> = {
  0: "left",
  1: "middle",
  2: "right",
};

/** Wire an element (the <video> showing the surface) so the guest's pointer and
 *  keyboard become InputEvents. Coordinates are normalised to the element, so the
 *  host maps them onto its surface regardless of resolution. Returns a detach fn.
 *  (keyCode is an approximate Windows VK; a precise code->VK map is a refinement.) */
export function attachInputCapture(el: HTMLElement, guest: GuestSession): () => void {
  const norm = (e: MouseEvent) => {
    const r = el.getBoundingClientRect();
    return {
      x: r.width ? (e.clientX - r.left) / r.width : 0,
      y: r.height ? (e.clientY - r.top) / r.height : 0,
    };
  };
  const onMove = (e: MouseEvent) => {
    const { x, y } = norm(e);
    guest.sendInput({ kind: "move", x, y });
  };
  const onDown = (e: MouseEvent) => {
    const b = MOUSE_BUTTON[e.button];
    if (b) guest.sendInput({ kind: "button", button: b, down: true });
  };
  const onUp = (e: MouseEvent) => {
    const b = MOUSE_BUTTON[e.button];
    if (b) guest.sendInput({ kind: "button", button: b, down: false });
  };
  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    guest.sendInput({ kind: "scroll", dy: e.deltaY > 0 ? -1 : 1 });
  };
  const onKeyDown = (e: KeyboardEvent) => {
    e.preventDefault();
    guest.sendInput({ kind: "key", vk: e.keyCode, down: true });
  };
  const onKeyUp = (e: KeyboardEvent) => {
    e.preventDefault();
    guest.sendInput({ kind: "key", vk: e.keyCode, down: false });
  };
  const onContext = (e: Event) => e.preventDefault();

  el.addEventListener("mousemove", onMove);
  el.addEventListener("mousedown", onDown);
  el.addEventListener("mouseup", onUp);
  el.addEventListener("wheel", onWheel, { passive: false });
  el.addEventListener("contextmenu", onContext);
  el.addEventListener("keydown", onKeyDown);
  el.addEventListener("keyup", onKeyUp);
  el.setAttribute("tabindex", "0"); // so the element can receive key events

  return () => {
    el.removeEventListener("mousemove", onMove);
    el.removeEventListener("mousedown", onDown);
    el.removeEventListener("mouseup", onUp);
    el.removeEventListener("wheel", onWheel);
    el.removeEventListener("contextmenu", onContext);
    el.removeEventListener("keydown", onKeyDown);
    el.removeEventListener("keyup", onKeyUp);
  };
}

// ── session hub: one host + one guest session, wired to the relay ────────────
// Routes namespaced ("wss") signals to the right side, and exposes the small set
// of host controls. Every control goes through the backend gate — the UI never
// injects input or bypasses can_send_input().
class SessionHub {
  private host: HostSession | null = null;
  private hostGuestUid: number | null = null;
  private hostGuestId = "";
  private hostWorkspace = "";
  private guest: GuestSession | null = null;
  private guestHostUid: number | null = null;
  private send: Signaler = () => {};
  private onStream: (s: MediaStream | null) => void = () => {};

  setSignaler(fn: Signaler) {
    this.send = fn;
  }
  setStreamListener(fn: (s: MediaStream | null) => void) {
    this.onStream = fn;
  }
  guestSession() {
    return this.guest;
  }
  sharing() {
    return !!this.host;
  }

  async startShare(
    workspace: string,
    guestUid: number,
    guestId: string,
    guestName: string,
    surface: SurfaceProvider,
  ) {
    this.stopHost();
    // Register the guest in the gate (as Viewer) BEFORE any input can arrive.
    try {
      await invoke("remote_join", { workspace, guest: guestId, name: guestName });
    } catch {
      /* backend unavailable */
    }
    this.host = new HostSession(workspace, guestUid, guestId, this.send);
    this.hostGuestUid = guestUid;
    this.hostGuestId = guestId;
    this.hostWorkspace = workspace;
    await this.host.share(surface);
  }

  async grant() {
    if (this.hostWorkspace) {
      await invoke("remote_grant", { workspace: this.hostWorkspace, guest: this.hostGuestId }).catch(
        () => {},
      );
    }
  }
  async revoke() {
    if (this.hostWorkspace) {
      await invoke("remote_revoke", { workspace: this.hostWorkspace }).catch(() => {});
    }
  }
  async sessionState(): Promise<string> {
    if (!this.hostWorkspace) return "{}";
    return invoke<string>("remote_session", { workspace: this.hostWorkspace }).catch(() => "{}");
  }

  disconnectHost() {
    const { hostWorkspace: ws, hostGuestId: guest, hostGuestUid: uid } = this;
    if (ws) invoke("remote_disconnect", { workspace: ws, guest }).catch(() => {});
    if (uid != null) this.send(uid, { ns: "wss", workspace: ws, t: "end" });
    this.stopHost();
  }
  private stopHost() {
    this.host?.stop();
    this.host = null;
    this.hostGuestUid = null;
    this.hostGuestId = "";
    this.hostWorkspace = "";
  }

  leaveGuest() {
    if (this.guestHostUid != null) {
      this.send(this.guestHostUid, { ns: "wss", workspace: "", t: "end" });
    }
    this.stopGuest();
  }
  private stopGuest() {
    this.guest?.stop();
    this.guest = null;
    this.guestHostUid = null;
    this.onStream(null);
  }

  async onSignal(from: number, sig: SessionSignal) {
    if (sig.t === "end") {
      if (this.guestHostUid === from) this.stopGuest();
      if (this.hostGuestUid === from) {
        invoke("remote_disconnect", {
          workspace: this.hostWorkspace,
          guest: this.hostGuestId,
        }).catch(() => {});
        this.stopHost();
      }
      return;
    }
    if (sig.t === "offer") {
      // A host is sharing a workspace with me -> guest.
      this.stopGuest();
      this.guest = new GuestSession(sig.workspace, from, this.send, (s) => this.onStream(s));
      this.guestHostUid = from;
      await this.guest.onSignal(sig);
      return;
    }
    if (this.host && this.hostGuestUid === from) await this.host.onSignal(sig);
    if (this.guest && this.guestHostUid === from) await this.guest.onSignal(sig);
  }
}

export const sessionHub = new SessionHub();
