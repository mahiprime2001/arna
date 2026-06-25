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
  /** The remote screen as a live H.264 video track (bind to a <video>). */
  const videoStream = ref<MediaStream | null>(null);
  /** One-time session code echoed back by the agent on accept (display only). */
  const sessionCode = ref<string | null>(null);
  /** Whether files can be sent (the `files` channel is open). */
  const canSendFiles = ref(false);
  /** Current upload progress 0..1 (0 when idle). */
  const uploadProgress = ref(0);
  /** Short upload status line, e.g. "sending report.pdf…" / "saved on remote". */
  const uploadStatus = ref("");

  let ws: WebSocket | null = null;
  let pc: RTCPeerConnection | null = null;
  let inputCh: RTCDataChannel | null = null;
  let filesCh: RTCDataChannel | null = null;

  /** Send an input event to the agent (no-op until the control channel is open). */
  function sendInput(event: Record<string, unknown>) {
    if (inputCh && inputCh.readyState === "open") {
      inputCh.send(JSON.stringify(event));
    }
  }

  /**
   * Send a file to the remote PC over the `files` channel: a `file_start`
   * control message, binary chunks (throttled by bufferedAmount), then
   * `file_end`. The agent saves it to ~/ArnaRemote/Incoming and acks.
   */
  async function sendFile(file: File) {
    if (!filesCh || filesCh.readyState !== "open") return;
    const ch = filesCh;
    const id = Date.now();
    const CHUNK = 16 * 1024;
    const HIGH_WATER = 1 * 1024 * 1024; // pause sending above 1 MB buffered

    ch.send(JSON.stringify({ t: "file_start", id, name: file.name, size: file.size }));
    uploadStatus.value = `sending ${file.name}…`;
    uploadProgress.value = 0;

    let offset = 0;
    while (offset < file.size) {
      // Backpressure: let the buffer drain before queueing more.
      while (ch.bufferedAmount > HIGH_WATER) {
        await new Promise((r) => setTimeout(r, 20));
      }
      const slice = file.slice(offset, offset + CHUNK);
      const buf = await slice.arrayBuffer();
      ch.send(buf);
      offset += buf.byteLength;
      uploadProgress.value = file.size ? offset / file.size : 1;
    }
    ch.send(JSON.stringify({ t: "file_end", id }));
    uploadStatus.value = `sent ${file.name} — finishing…`;
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

      // The agent sends the screen as an H.264 video track.
      pc.addTransceiver("video", { direction: "recvonly" });
      pc.ontrack = (e) => {
        videoStream.value = e.streams[0] ?? new MediaStream([e.track]);
        status.value = "streaming";
      };

      // Control channel: we send mouse/keyboard events to the agent.
      inputCh = pc.createDataChannel("input");
      inputCh.onopen = () => (canControl.value = true);
      inputCh.onclose = () => (canControl.value = false);

      // Files channel: push files to the remote PC; the agent acks file_done.
      filesCh = pc.createDataChannel("files");
      filesCh.onopen = () => (canSendFiles.value = true);
      filesCh.onclose = () => (canSendFiles.value = false);
      filesCh.onmessage = (ev) => {
        try {
          const m = JSON.parse(ev.data);
          if (m.t === "file_done") {
            uploadStatus.value = `saved on remote: ${m.name}`;
            uploadProgress.value = 0;
          }
        } catch {
          /* ignore non-JSON */
        }
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
    canSendFiles.value = false;
    uploadProgress.value = 0;
    uploadStatus.value = "";
    inputCh = null;
    filesCh = null;
    if (pc) {
      pc.close();
      pc = null;
    }
    if (ws) {
      ws.close();
      ws = null;
    }
    videoStream.value = null;
    sessionCode.value = null;
    status.value = "idle";
  }

  onUnmounted(disconnect);

  return {
    status,
    active,
    connected,
    canControl,
    videoStream,
    sessionCode,
    canSendFiles,
    uploadProgress,
    uploadStatus,
    connect,
    disconnect,
    sendInput,
    sendFile,
  };
}
