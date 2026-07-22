// Peer-to-peer calling. Media flows directly between the two devices (WebRTC);
// only the tiny signaling (offer/answer/ICE) goes through the relay.
//
// One active call at a time. STUN handles most NATs; cross-internet symmetric
// NATs would additionally need a TURN server (not configured yet).

export type CallStatus = "idle" | "outgoing" | "incoming" | "connected";
export type CallKind = "audio" | "video";

export interface CallState {
  status: CallStatus;
  peerId: number | null;
  peerName: string;
  kind: CallKind;
  muted: boolean;
  camOff: boolean;
  localStream: MediaStream | null;
  remoteStream: MediaStream | null;
  error: string | null;
}

const ICE: RTCIceServer[] = [{ urls: "stun:stun.l.google.com:19302" }];

const idle: CallState = {
  status: "idle",
  peerId: null,
  peerName: "",
  kind: "audio",
  muted: false,
  camOff: false,
  localStream: null,
  remoteStream: null,
  error: null,
};

class CallEngine {
  private pc: RTCPeerConnection | null = null;
  private local: MediaStream | null = null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private pendingOffer: any = null;
  private state: CallState = { ...idle };

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private send: (to: number, signal: any) => void = () => {};
  private resolveName: (id: number) => string = (id) => `#${id}`;
  private emit: (s: CallState) => void = () => {};

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  setSignaler(fn: (to: number, signal: any) => void) {
    this.send = fn;
  }
  setResolveName(fn: (id: number) => string) {
    this.resolveName = fn;
  }
  setListener(fn: (s: CallState) => void) {
    this.emit = fn;
  }

  private push(patch: Partial<CallState>) {
    this.state = { ...this.state, ...patch };
    this.emit(this.state);
  }

  async start(peerId: number, name: string, kind: CallKind) {
    if (this.state.status !== "idle") return;
    this.push({ status: "outgoing", peerId, peerName: name, kind, muted: false, camOff: false });
    if (!(await this.acquire(kind))) return;
    this.makePc(peerId);
    this.attachLocal(kind);
    const offer = await this.pc!.createOffer();
    await this.pc!.setLocalDescription(offer);
    this.send(peerId, { t: "offer", sdp: offer, kind });
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  async onSignal(from: number, sig: any) {
    if (!sig) return;
    switch (sig.t) {
      case "offer":
        if (this.state.status !== "idle") {
          this.send(from, { t: "decline" });
          return;
        }
        this.pendingOffer = sig.sdp;
        this.push({
          status: "incoming",
          peerId: from,
          peerName: this.resolveName(from),
          kind: sig.kind || "audio",
        });
        break;
      case "answer":
        if (this.pc) await this.pc.setRemoteDescription(sig.sdp);
        this.push({ status: "connected" });
        break;
      case "ice":
        if (this.pc && sig.candidate) {
          try {
            await this.pc.addIceCandidate(sig.candidate);
          } catch {
            /* ignore late candidates */
          }
        }
        break;
      case "decline":
      case "end":
        this.cleanup();
        break;
    }
  }

  async accept() {
    if (this.state.status !== "incoming" || !this.pendingOffer || this.state.peerId == null) return;
    const peerId = this.state.peerId;
    if (!(await this.acquire(this.state.kind))) return;
    this.makePc(peerId);
    this.attachLocal(this.state.kind);
    await this.pc!.setRemoteDescription(this.pendingOffer);
    const answer = await this.pc!.createAnswer();
    await this.pc!.setLocalDescription(answer);
    this.send(peerId, { t: "answer", sdp: answer });
    this.pendingOffer = null;
    this.push({ status: "connected" });
  }

  decline() {
    if (this.state.peerId != null) this.send(this.state.peerId, { t: "decline" });
    this.cleanup();
  }
  hangup() {
    if (this.state.peerId != null) this.send(this.state.peerId, { t: "end" });
    this.cleanup();
  }

  toggleMic() {
    const a = this.local?.getAudioTracks()[0];
    if (a) {
      a.enabled = !a.enabled;
      this.push({ muted: !a.enabled });
    }
  }
  toggleCam() {
    const v = this.local?.getVideoTracks()[0];
    if (v) {
      v.enabled = !v.enabled;
      this.push({ camOff: !v.enabled });
    }
  }

  dismissError() {
    if (this.state.error) this.cleanup();
  }

  private fail(message: string) {
    // Keep the overlay up so the user sees why, instead of a silent no-op.
    this.push({ error: message });
  }

  // Attach whatever local media we have, and negotiate receive-only for any
  // device we lack, so we still RECEIVE the other side. Missing mic -> we still
  // hear them; missing camera -> we still see them.
  private attachLocal(kind: CallKind) {
    const haveAudio = !!this.local?.getAudioTracks().length;
    const haveVideo = !!this.local?.getVideoTracks().length;
    this.local?.getTracks().forEach((t) => this.pc!.addTrack(t, this.local!));
    if (!haveAudio) this.pc!.addTransceiver("audio", { direction: "recvonly" });
    if (kind === "video" && !haveVideo) {
      this.pc!.addTransceiver("video", { direction: "recvonly" });
    }
  }

  // Try progressively looser device requests so a missing mic or camera doesn't
  // block the call. Permission denial is the only hard stop.
  private async acquire(kind: CallKind): Promise<boolean> {
    if (!navigator.mediaDevices?.getUserMedia) {
      this.fail(
        "Calls need a secure connection. Open Arna on this computer (localhost), or over HTTPS. Camera and mic are blocked on plain http:// LAN addresses.",
      );
      return false;
    }
    const tries: MediaStreamConstraints[] =
      kind === "video"
        ? [{ audio: true, video: true }, { audio: false, video: true }, { audio: true, video: false }]
        : [{ audio: true, video: false }];

    for (const constraints of tries) {
      try {
        this.local = await navigator.mediaDevices.getUserMedia(constraints);
        this.push({
          localStream: this.local,
          muted: this.local.getAudioTracks().length === 0,
          camOff: kind === "video" && this.local.getVideoTracks().length === 0,
        });
        return true;
      } catch (e) {
        if (e instanceof DOMException && e.name === "NotAllowedError") {
          this.fail("Microphone/camera permission was blocked. Allow it in the browser and try again.");
          return false;
        }
        // otherwise fall through to the next, looser attempt
      }
    }

    // No usable mic or camera: still join, receive-only.
    this.local = null;
    this.push({ localStream: null, muted: true, camOff: kind === "video" });
    return true;
  }

  private makePc(peerId: number) {
    const pc = new RTCPeerConnection({ iceServers: ICE });
    this.pc = pc;
    pc.onicecandidate = (e) => {
      if (e.candidate) this.send(peerId, { t: "ice", candidate: e.candidate });
    };
    pc.ontrack = (e) => {
      this.push({ remoteStream: e.streams[0] });
    };
  }

  private cleanup() {
    this.local?.getTracks().forEach((t) => t.stop());
    try {
      this.pc?.close();
    } catch {
      /* ignore */
    }
    this.pc = null;
    this.local = null;
    this.pendingOffer = null;
    this.state = { ...idle };
    this.emit(this.state);
  }
}

export const callEngine = new CallEngine();
