import { onUnmounted, ref } from "vue";

/**
 * Remote-screen session: connects to the signaling backend, asks the agent for
 * consent (the operator must accept), then negotiates a WebRTC peer connection
 * and exposes the latest screen frame as an object URL.
 */
export function useRemote() {
  const status = ref("idle");
  const active = ref(false);
  const connected = ref(false);
  const canControl = ref(false);
  const screenUrl = ref<string | null>(null);
  /** One-time session code echoed back by the agent on accept (display only). */
  const sessionCode = ref<string | null>(null);

  let ws: WebSocket | null = null;
  let pc: RTCPeerConnection | null = null;
  let inputCh: RTCDataChannel | null = null;
  let lastUrl: string | null = null;

  /** Send an input event to the agent (no-op until the control channel is open). */
  function sendInput(event: Record<string, unknown>) {
    if (inputCh && inputCh.readyState === "open") {
      inputCh.send(JSON.stringify(event));
    }
  }

  function connect(backendUrl: string, agentId: string, ticket?: string) {
    disconnect();
    active.value = true;
    const myId = "viewer-" + Math.floor(Math.random() * 1e6);

    ws = new WebSocket(backendUrl);
    ws.onopen = () => {
      ws!.send(JSON.stringify({ type: "register", role: "console", id: myId }));
      // Ask for consent first; we only send the WebRTC offer once accepted.
      ws!.send(JSON.stringify({ type: "connect_request", to: agentId, ticket: ticket || undefined }));
      status.value = "requesting access…";
    };
    ws.onerror = () => (status.value = "connection error");
    ws.onclose = () => {
      if (active.value) status.value = "disconnected";
      connected.value = false;
    };
    ws.onmessage = async (ev) => {
      const m = JSON.parse(ev.data);
      if (m.type === "request_denied") {
        status.value = `denied: ${m.reason}`;
        active.value = false;
      } else if (m.type === "signal") {
        const d = m.data;
        if (d.kind === "consent") {
          if (d.accepted) {
            sessionCode.value = d.code ?? null;
            status.value = "accepted — connecting";
            offer(agentId);
          } else {
            status.value = `declined: ${d.reason ?? "by operator"}`;
            active.value = false;
          }
        } else if (d.kind === "answer") {
          await pc!.setRemoteDescription({ type: "answer", sdp: d.sdp });
        } else if (d.kind === "ice") {
          try {
            await pc!.addIceCandidate(d.candidate);
          } catch (e) {
            console.warn("addIceCandidate failed", e);
          }
        }
      } else if (m.type === "peer_offline") {
        status.value = `agent "${m.to}" is offline`;
      } else if (m.type === "error") {
        status.value = "error: " + m.message;
      }
    };

    async function offer(agentId: string) {
      pc = new RTCPeerConnection({ iceServers: [{ urls: "stun:stun.l.google.com:19302" }] });
      pc.onicecandidate = (e) => {
        if (e.candidate) {
          ws!.send(
            JSON.stringify({ type: "signal", to: agentId, data: { kind: "ice", candidate: e.candidate.toJSON() } }),
          );
        }
      };
      pc.onconnectionstatechange = () => {
        const s = pc!.connectionState;
        connected.value = s === "connected";
        if (s !== "connected") status.value = s;
      };

      // Control channel: we send mouse/keyboard events to the agent.
      inputCh = pc.createDataChannel("input");
      inputCh.onopen = () => (canControl.value = true);
      inputCh.onclose = () => (canControl.value = false);

      const ch = pc.createDataChannel("screen");
      ch.binaryType = "arraybuffer";
      ch.onopen = () => (status.value = "streaming");
      ch.onmessage = (ev) => {
        const blob = new Blob([ev.data], { type: "image/jpeg" });
        const url = URL.createObjectURL(blob);
        if (lastUrl) URL.revokeObjectURL(lastUrl);
        lastUrl = url;
        screenUrl.value = url;
      };

      const off = await pc.createOffer();
      await pc.setLocalDescription(off);
      ws!.send(JSON.stringify({ type: "signal", to: agentId, data: { kind: "offer", sdp: off.sdp } }));
    }
  }

  function disconnect() {
    active.value = false;
    connected.value = false;
    canControl.value = false;
    inputCh = null;
    if (pc) {
      pc.close();
      pc = null;
    }
    if (ws) {
      ws.close();
      ws = null;
    }
    if (lastUrl) {
      URL.revokeObjectURL(lastUrl);
      lastUrl = null;
    }
    screenUrl.value = null;
    sessionCode.value = null;
    status.value = "idle";
  }

  onUnmounted(disconnect);

  return { status, active, connected, canControl, screenUrl, sessionCode, connect, disconnect, sendInput };
}
