// Peer-to-peer calling. Media flows directly between the two devices (WebRTC);
// only the tiny signaling (offer/answer/ICE) goes through the relay.
//
// One active call at a time. STUN handles most NATs; cross-internet symmetric
// NATs would additionally need a TURN server (not configured yet).

export type CallStatus = "idle" | "outgoing" | "incoming" | "connecting" | "connected";
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

// STUN is enough when at least one side is reachable. Symmetric NATs (common on
// mobile data, some corporate/ISP routers) need a TURN relay -- set the three
// VITE_ARNA_TURN_* vars to add one without touching this file.
const envv = import.meta.env;

const ICE: RTCIceServer[] = [
  { urls: "stun:stun.l.google.com:19302" },
  { urls: "stun:stun1.l.google.com:19302" },
  ...(envv.VITE_ARNA_TURN_URL
    ? [
        {
          urls: envv.VITE_ARNA_TURN_URL,
          username: envv.VITE_ARNA_TURN_USERNAME,
          credential: envv.VITE_ARNA_TURN_CREDENTIAL,
        } as RTCIceServer,
      ]
    : []),
];

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
  private remote: MediaStream | null = null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private pendingOffer: any = null;
  // ICE candidates that arrived before we had a peer connection with a remote
  // description to attach them to -- typically the caller's candidates landing
  // while the callee is still looking at the ringing screen. Dropping these
  // leaves the callee with no route to the caller, so the call "connects" but
  // no media ever flows. Hold them and flush once we can accept them.
  private pendingIce: RTCIceCandidateInit[] = [];
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
        if (this.pc) {
          await this.pc.setRemoteDescription(sig.sdp);
          await this.flushIce();
        }
        // Not "connected" yet -- that is decided by the actual ICE connection,
        // in makePc(). Claiming it here is what made dead calls look live.
        this.push({ status: "connecting" });
        break;
      case "ice":
        if (sig.candidate) await this.addIce(sig.candidate);
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
    const kind = this.state.kind;

    // Order matters: take the offer FIRST so its m-lines exist, then add our
    // tracks into those transceivers. Adding tracks first builds a different
    // shape than the caller offered and the two sides fail to line up.
    this.makePc(peerId);
    await this.pc!.setRemoteDescription(this.pendingOffer);
    this.pendingOffer = null;
    await this.flushIce();

    if (!(await this.acquire(kind))) return;
    this.attachLocal(kind);

    const answer = await this.pc!.createAnswer();
    await this.pc!.setLocalDescription(answer);
    this.send(peerId, { t: "answer", sdp: answer });
    this.push({ status: "connecting" });
  }

  private async addIce(candidate: RTCIceCandidateInit) {
    // Only safe to add once a remote description exists; otherwise hold it.
    if (!this.pc || !this.pc.remoteDescription) {
      this.pendingIce.push(candidate);
      return;
    }
    try {
      await this.pc.addIceCandidate(candidate);
    } catch {
      /* a stale candidate for a closed connection is harmless */
    }
  }

  private async flushIce() {
    const queued = this.pendingIce;
    this.pendingIce = [];
    for (const c of queued) {
      try {
        await this.pc?.addIceCandidate(c);
      } catch {
        /* ignore */
      }
    }
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

    // Only the caller adds these. Answering, the offer's m-lines already exist
    // (setRemoteDescription created them) and default to receive-only, so
    // adding more here would put extra m-lines in the answer -- which the
    // caller rejects, killing the call for anyone without a mic or camera.
    if (this.pc!.remoteDescription) return;

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
    this.remote = new MediaStream();

    pc.onicecandidate = (e) => {
      if (e.candidate) this.send(peerId, { t: "ice", candidate: e.candidate });
    };

    // Collect tracks ourselves rather than trusting e.streams[0], which is empty
    // when the other side negotiated a transceiver without an attached stream.
    pc.ontrack = (e) => {
      if (pc !== this.pc) return;
      this.remote!.addTrack(e.track);
      this.push({ remoteStream: this.remote });
    };

    // The call is "connected" when media can actually flow, not when we sent an
    // answer. If ICE gives up, say so instead of showing a silent dead call.
    pc.onconnectionstatechange = () => {
      if (pc !== this.pc) return;
      if (pc.connectionState === "connected") {
        this.push({ status: "connected" });
      } else if (pc.connectionState === "failed") {
        this.fail(
          "Couldn't connect the call. This usually means both devices are behind " +
            "networks that block direct connections, which needs a TURN relay.",
        );
      }
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
    this.remote = null;
    this.pendingOffer = null;
    this.pendingIce = [];
    this.state = { ...idle };
    this.emit(this.state);
  }
}

export const callEngine = new CallEngine();
