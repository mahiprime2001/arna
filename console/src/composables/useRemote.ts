import { onUnmounted, ref } from "vue";

/**
 * Remote-screen session: connects to the signaling backend, negotiates a WebRTC
 * peer connection with the agent, and exposes the latest screen frame as an
 * object URL. (Phase 1c — view only; control comes next.)
 */
export function useRemote() {
  const status = ref("idle");
  const active = ref(false);
  const connected = ref(false);
  const screenUrl = ref<string | null>(null);

  let ws: WebSocket | null = null;
  let pc: RTCPeerConnection | null = null;
  let lastUrl: string | null = null;

  function connect(backendUrl: string, agentId: string) {
    disconnect();
    active.value = true;
    const myId = "viewer-" + Math.floor(Math.random() * 1e6);

    ws = new WebSocket(backendUrl);
    ws.onopen = () => {
      ws!.send(JSON.stringify({ type: "register", role: "console", id: myId }));
      status.value = "connecting";
      offer(agentId);
    };
    ws.onerror = () => (status.value = "connection error");
    ws.onclose = () => {
      if (active.value) status.value = "disconnected";
      connected.value = false;
    };
    ws.onmessage = async (ev) => {
      const m = JSON.parse(ev.data);
      if (m.type === "signal") {
        const d = m.data;
        if (d.kind === "answer") {
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
    status.value = "idle";
  }

  onUnmounted(disconnect);

  return { status, active, connected, screenUrl, connect, disconnect };
}
