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
  /** A clear, user-facing failure reason (offline / declined / unreachable), or null. */
  const errorMessage = ref<string | null>(null);
  /** Coarse error kind so the UI can pick an icon: "offline" | "denied" | null. */
  const errorKind = ref<"offline" | "denied" | null>(null);
  /** True when the remote PC requires the caller to type the operator's code. */
  const awaitingCode = ref(false);
  /** Error shown under the code field (wrong code). */
  const codeError = ref("");
  /** Whether files can be sent (the `files` channel is open). */
  const canSendFiles = ref(false);
  /** Current upload progress 0..1 (0 when idle). */
  const uploadProgress = ref(0);
  /** Short upload status line, e.g. "sending report.pdf…" / "saved on remote". */
  const uploadStatus = ref("");
  /** Current download progress 0..1 (0 when idle). */
  const downloadProgress = ref(0);
  /** Short download status line. */
  const downloadStatus = ref("");
  /** Reassembly buffer for an in-progress download. */
  let dl: { name: string; size: number; received: number; chunks: Uint8Array[] } | null = null;
  /** Whether chat is available (the `chat` channel is open). */
  const canChat = ref(false);
  /** In-session chat log. */
  const messages = ref<{ mine: boolean; text: string; ts: number }[]>([]);
  /** Unread incoming messages (reset via markChatRead). */
  const unread = ref(0);
  /** Monitors on the remote PC, announced by the agent on the input channel. */
  const monitors = ref<{ index: number; label: string; width: number; height: number; primary: boolean }[]>([]);
  /** Index of the monitor currently being streamed. */
  const currentMonitor = ref(0);

  let ws: WebSocket | null = null;
  let pc: RTCPeerConnection | null = null;
  let inputCh: RTCDataChannel | null = null;
  let filesCh: RTCDataChannel | null = null;
  let chatCh: RTCDataChannel | null = null;
  let currentAgentId: string | null = null;

  /** Submit the operator's code (require-code consent mode). */
  function submitCode(code: string) {
    const c = code.trim();
    if (!ws || !currentAgentId || !c) return;
    codeError.value = "";
    ws.send(JSON.stringify({ type: "signal", to: currentAgentId, data: { kind: "code", code: c } }));
  }

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

  /** Ask the remote PC for a file; the operator there picks which one to send. */
  function requestDownload() {
    if (!filesCh || filesCh.readyState !== "open") return;
    filesCh.send(JSON.stringify({ t: "dl_request" }));
    downloadStatus.value = "waiting for the other PC to choose a file…";
    downloadProgress.value = 0;
  }

  /** Assemble received chunks and trigger a browser "Save as". */
  function finishDownload() {
    if (!dl) return;
    const blob = new Blob(dl.chunks as BlobPart[], { type: "application/octet-stream" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = dl.name;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    downloadStatus.value = `downloaded ${dl.name}`;
    downloadProgress.value = 0;
    dl = null;
  }

  /** Send a chat message to the remote PC. */
  function sendChat(text: string) {
    const body = text.trim();
    if (!body || !chatCh || chatCh.readyState !== "open") return;
    chatCh.send(JSON.stringify({ t: "msg", text: body, ts: Date.now() }));
    messages.value.push({ mine: true, text: body, ts: Date.now() });
  }

  /** Clear the unread counter (call when the chat panel is open/focused). */
  function markChatRead() {
    unread.value = 0;
  }

  function connect(backendUrl: string, agentId: string, ticket?: string) {
    disconnect();
    active.value = true;
    currentAgentId = agentId;
    const myId = "viewer-" + Math.floor(Math.random() * 1e6);

    // STUN/TURN servers; replaced by the backend's `registered` reply (which
    // arrives before we send the offer) so the whole deployment shares one relay.
    let iceServers: RTCIceServer[] = [{ urls: "stun:stun.l.google.com:19302" }];

    ws = new WebSocket(backendUrl);
    ws.onopen = () => {
      ws!.send(JSON.stringify({ type: "register", role: "console", id: myId }));
      // Ask for consent first; we only send the WebRTC offer once accepted.
      ws!.send(JSON.stringify({ type: "connect_request", to: agentId, ticket: ticket || undefined }));
      status.value = "requesting access…";
    };
    /** Record a clear failure and drop back to the idle panel. */
    function fail(message: string, kind: "offline" | "denied" | null = null) {
      errorMessage.value = message;
      errorKind.value = kind;
      status.value = "idle";
      active.value = false;
    }

    ws.onerror = () => {
      if (!connected.value) fail("Can't reach the server. Check the address and that the backend is running.");
    };
    ws.onclose = () => {
      if (connected.value) fail(`Connection to "${agentId}" was lost.`);
      connected.value = false;
    };
    ws.onmessage = async (ev) => {
      const m = JSON.parse(ev.data);
      if (m.type === "registered") {
        if (Array.isArray(m.ice_servers) && m.ice_servers.length) {
          iceServers = m.ice_servers
            .filter((s: any) => Array.isArray(s.urls) && s.urls.length)
            .map((s: any) => ({ urls: s.urls, username: s.username, credential: s.credential }));
        }
      } else if (m.type === "request_denied") {
        const reason = String(m.reason ?? "");
        if (reason.includes("offline")) {
          fail(`"${agentId}" is offline. Make sure the agent app is running on that PC.`, "offline");
        } else if (reason.includes("auth") || reason.includes("ticket")) {
          fail(`Access denied — ${reason}.`, "denied");
        } else {
          fail(`Couldn't connect — ${reason}.`, "denied");
        }
      } else if (m.type === "signal") {
        const d = m.data;
        if (d.kind === "consent") {
          if (d.accepted && d.require_code) {
            // The operator must read you a code; show the entry prompt.
            awaitingCode.value = true;
            codeError.value = "";
            status.value = "enter the code shown on the other PC";
          } else if (d.accepted) {
            sessionCode.value = d.code ?? null;
            status.value = "accepted — connecting";
            offer(agentId);
          } else {
            fail(d.reason ? `The remote PC declined: ${d.reason}.` : "The remote PC declined the connection.", "denied");
          }
        } else if (d.kind === "code_ok") {
          awaitingCode.value = false;
          status.value = "accepted — connecting";
          offer(agentId);
        } else if (d.kind === "code_bad") {
          if (d.final) {
            awaitingCode.value = false;
            fail("Wrong code too many times — connection refused.", "denied");
          } else {
            codeError.value = "That code didn't match — try again.";
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
        fail(`"${m.to}" went offline.`, "offline");
      } else if (m.type === "error") {
        fail(`Server error: ${m.message}.`, "denied");
      }
    };

    async function offer(agentId: string) {
      pc = new RTCPeerConnection({ iceServers });
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

      // Control channel: we send mouse/keyboard events to the agent, and the
      // agent sends back the list of monitors we can switch between.
      inputCh = pc.createDataChannel("input");
      inputCh.onopen = () => (canControl.value = true);
      inputCh.onclose = () => (canControl.value = false);
      inputCh.onmessage = (ev) => {
        try {
          const m = JSON.parse(ev.data);
          if (m.t === "monitors" && Array.isArray(m.list)) {
            monitors.value = m.list;
            const primary = m.list.find((s: any) => s.primary);
            currentMonitor.value = primary ? primary.index : m.list[0]?.index ?? 0;
          }
        } catch {
          /* ignore non-JSON */
        }
      };

      // Files channel: upload to (text + binary) and download from the remote PC.
      filesCh = pc.createDataChannel("files");
      filesCh.binaryType = "arraybuffer";
      filesCh.onopen = () => (canSendFiles.value = true);
      filesCh.onclose = () => (canSendFiles.value = false);
      filesCh.onmessage = (ev) => {
        if (typeof ev.data !== "string") {
          // Binary = a download chunk.
          if (!dl) return;
          const u8 = new Uint8Array(ev.data);
          dl.chunks.push(u8);
          dl.received += u8.byteLength;
          downloadProgress.value = dl.size ? dl.received / dl.size : 0;
          return;
        }
        try {
          const m = JSON.parse(ev.data);
          if (m.t === "file_done") {
            uploadStatus.value = `saved on remote: ${m.name}`;
            uploadProgress.value = 0;
          } else if (m.t === "dl_start") {
            dl = { name: m.name, size: m.size ?? 0, received: 0, chunks: [] };
            downloadStatus.value = `downloading ${m.name}…`;
            downloadProgress.value = 0;
          } else if (m.t === "dl_end") {
            finishDownload();
          } else if (m.t === "dl_cancel") {
            dl = null;
            downloadProgress.value = 0;
            downloadStatus.value =
              m.reason === "cancelled" ? "the other PC cancelled the download" : "download failed";
          }
        } catch {
          /* ignore non-JSON */
        }
      };

      // Chat channel: live text both ways during the session.
      chatCh = pc.createDataChannel("chat");
      chatCh.onopen = () => (canChat.value = true);
      chatCh.onclose = () => (canChat.value = false);
      chatCh.onmessage = (ev) => {
        try {
          const m = JSON.parse(ev.data);
          if (m.t === "msg" && typeof m.text === "string") {
            messages.value.push({ mine: false, text: m.text, ts: m.ts ?? Date.now() });
            unread.value += 1;
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
    downloadProgress.value = 0;
    downloadStatus.value = "";
    dl = null;
    canChat.value = false;
    messages.value = [];
    unread.value = 0;
    monitors.value = [];
    currentMonitor.value = 0;
    errorMessage.value = null;
    errorKind.value = null;
    awaitingCode.value = false;
    codeError.value = "";
    currentAgentId = null;
    inputCh = null;
    filesCh = null;
    chatCh = null;
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

  /** Ask the remote PC to stream a different monitor. */
  function selectMonitor(i: number) {
    currentMonitor.value = i;
    sendInput({ t: "display", i });
  }

  onUnmounted(disconnect);

  return {
    status,
    active,
    connected,
    canControl,
    videoStream,
    sessionCode,
    errorMessage,
    errorKind,
    awaitingCode,
    codeError,
    submitCode,
    canSendFiles,
    uploadProgress,
    uploadStatus,
    downloadProgress,
    downloadStatus,
    canChat,
    messages,
    unread,
    monitors,
    currentMonitor,
    selectMonitor,
    connect,
    disconnect,
    sendInput,
    sendFile,
    requestDownload,
    sendChat,
    markChatRead,
  };
}
