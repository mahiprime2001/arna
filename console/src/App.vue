<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useRemote } from "./composables/useRemote";
import { useAuth, type Device, type DeviceInfo } from "./composables/useAuth";
import Icon from "./components/Icon.vue";
import QrCode from "./components/QrCode.vue";

// Remember the server/agent and recent machines so you don't retype them.
const STORE_KEY = "arna.console";
type Store = { backend?: string; agentId?: string; recents?: string[] };
function loadStore(): Store {
  try {
    return JSON.parse(localStorage.getItem(STORE_KEY) || "{}");
  } catch {
    return {};
  }
}
const saved = loadStore();

// Default server: a build can bake in the hosted backend via VITE_ARNA_BACKEND
// (set for production bundles); dev falls back to localhost. A saved value the
// user entered always wins.
const DEFAULT_BACKEND = import.meta.env.VITE_ARNA_BACKEND || "ws://127.0.0.1:8081/ws";
const backend = ref(saved.backend || DEFAULT_BACKEND);
const agentId = ref(saved.agentId || "agent-1");
const ticket = ref("");
const recents = ref<string[]>(saved.recents || []);

function persist() {
  try {
    localStorage.setItem(
      STORE_KEY,
      JSON.stringify({ backend: backend.value, agentId: agentId.value, recents: recents.value }),
    );
  } catch {
    /* storage unavailable — fine */
  }
}
function rememberAgent(id: string) {
  if (!id) return;
  recents.value = [id, ...recents.value.filter((x) => x !== id)].slice(0, 5);
}

const {
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
  apps,
  bubbleApp,
  openApp,
  exitBubble,
  pushClipboard,
  connect,
  disconnect,
  sendInput,
  sendFile,
  requestDownload,
  sendChat,
  markChatRead,
} = useRemote();

// Show whichever transfer (upload or download) is active.
const transferProgress = computed(() => Math.max(uploadProgress.value, downloadProgress.value));
const transferStatus = computed(() => downloadStatus.value || uploadStatus.value);

// ── Accounts ────────────────────────────────────────────────────────────────
const {
  email: accountEmail,
  userId,
  devices: myDevices,
  loggedIn,
  authError,
  busy: authBusy,
  token: sessionToken,
  login: authLogin,
  signup: authSignup,
  logout: authLogout,
  refreshDevices,
  addDevice: authAddDevice,
  fetchMe,
  setDevicePassword,
  lookupDevice,
} = useAuth();

const loginEmail = ref("");
const loginPassword = ref("");
const isSignup = ref(false);
const showManual = ref(false); // signed-out: connect without an account (self-host)
const showServer = ref(false); // signed-out: reveal the server address field

async function submitAuth() {
  const fn = isSignup.value ? authSignup : authLogin;
  await fn(backend.value.trim(), loginEmail.value.trim(), loginPassword.value);
  if (loggedIn.value) {
    loginPassword.value = "";
    fetchMe(backend.value.trim());
  }
}

// ── Sidebar / navigation ──────────────────────────────────────────────────
const nav = ref<"devices" | "profile">("devices");
const sidebarOpen = ref(true);
function go(to: "devices" | "profile") {
  nav.value = to;
  errorMessage.value = null;
}

// ── Connect flow (enter id → resolve name → password / wait) ───────────────
const connectId = ref("");
const connectInfo = ref<DeviceInfo | null>(null);
const connectResolving = ref(false);
const connectPassword = ref("");
const connectError = ref("");
const showConnectDialog = ref(false);

async function openConnect(id: string, known?: Device) {
  const trimmed = id.trim();
  if (!trimmed) return;
  connectId.value = trimmed;
  connectPassword.value = "";
  connectError.value = "";
  errorMessage.value = null;
  showConnectDialog.value = true;
  if (known) {
    connectInfo.value = { id: known.id, name: known.name, has_password: !!known.has_password };
    return;
  }
  connectResolving.value = true;
  connectInfo.value = null;
  const info = await lookupDevice(backend.value.trim(), trimmed);
  connectResolving.value = false;
  // Unknown id: still allow a request — the operator can accept.
  connectInfo.value = info ?? { id: trimmed, name: trimmed, has_password: false };
}

function doConnect(withPassword: boolean) {
  const id = connectId.value.trim();
  if (!id) return;
  rememberAgent(id);
  persist();
  showConnectDialog.value = false;
  connect(
    backend.value.trim(),
    id,
    sessionToken.value || ticket.value.trim() || undefined,
    withPassword ? connectPassword.value : undefined,
  );
}

// A wrong/needed password bounces back here — reopen the dialog with the message.
watch(active, (a) => {
  if (!a && errorKind.value === "password") {
    showConnectDialog.value = true;
    connectError.value = errorMessage.value || "Wrong password.";
  }
});

// Manual connect (self-host, signed out): server + agent id + optional ticket.
function manualConnect() {
  const id = agentId.value.trim();
  if (!id) return;
  rememberAgent(id);
  persist();
  connect(backend.value.trim(), id, ticket.value.trim() || undefined);
}

// ── Add a device ───────────────────────────────────────────────────────────
const showAddDevice = ref(false);
const newDeviceId = ref("");
const newDeviceName = ref("");
const newDeviceToken = ref("");
async function submitAddDevice() {
  const id = newDeviceId.value.trim();
  if (!id) return;
  const token = await authAddDevice(backend.value.trim(), id, newDeviceName.value.trim() || id);
  if (token) newDeviceToken.value = token;
}
function resetAddDevice() {
  showAddDevice.value = false;
  newDeviceId.value = "";
  newDeviceName.value = "";
  newDeviceToken.value = "";
}

// ── Per-device access password ──────────────────────────────────────────────
const pwDevice = ref<Device | null>(null);
const pwInput = ref("");
const pwBusy = ref(false);
const pwError = ref("");
function openPw(d: Device) {
  pwDevice.value = d;
  pwInput.value = "";
  pwError.value = "";
}
async function savePw(clear = false) {
  if (!pwDevice.value) return;
  pwBusy.value = true;
  pwError.value = "";
  const ok = await setDevicePassword(
    backend.value.trim(),
    pwDevice.value.id,
    clear ? null : pwInput.value,
  );
  pwBusy.value = false;
  if (ok) pwDevice.value = null;
  else pwError.value = "Couldn't update. Try again.";
}

// ── Profile links + QR ───────────────────────────────────────────────────────
const linkBase = computed(() => {
  try {
    const u = new URL(backend.value);
    u.protocol = u.protocol === "wss:" ? "https:" : "http:";
    return u.origin;
  } catch {
    return location.origin;
  }
});
const connectLink = computed(() => `${linkBase.value}/?connect=${userId.value ?? ""}`);
const inviteLink = computed(() => `${linkBase.value}/?invite=${userId.value ?? ""}`);

const copied = ref("");
function copyText(t: string, label = "") {
  navigator.clipboard?.writeText(t).catch(() => {});
  copied.value = label || t;
  setTimeout(() => {
    if (copied.value === (label || t)) copied.value = "";
  }, 1500);
}

// Esc closes whichever modal is open (topmost first).
function onEsc(e: KeyboardEvent) {
  if (e.key !== "Escape") return;
  if (pwDevice.value) pwDevice.value = null;
  else if (showConnectDialog.value) showConnectDialog.value = false;
  else if (showAddDevice.value) resetAddDevice();
}

onMounted(() => {
  window.addEventListener("keydown", onEsc);
  if (loggedIn.value) {
    refreshDevices(backend.value.trim());
    fetchMe(backend.value.trim());
  }
  // Deep links: ?connect=<id> prefills the connect field; ?invite= opens signup.
  const params = new URLSearchParams(location.search);
  const c = params.get("connect");
  if (c) {
    nav.value = "devices";
    connectId.value = c;
  }
  if (params.get("invite")) isSignup.value = true;
});

// ── Live session refs/handlers (unchanged) ─────────────────────────────────
const videoEl = ref<HTMLVideoElement | null>(null);
const screenEl = ref<HTMLDivElement | null>(null);
const fileInput = ref<HTMLInputElement | null>(null);
const dragOver = ref(false);
let lastMove = 0;

function pickFile() {
  fileInput.value?.click();
}
function onFileChosen(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (file) sendFile(file);
  input.value = "";
}
function onDrop(e: DragEvent) {
  dragOver.value = false;
  if (!canSendFiles.value) return;
  const file = e.dataTransfer?.files?.[0];
  if (file) sendFile(file);
}
function onDragOver() {
  if (canSendFiles.value) dragOver.value = true;
}

watch(videoStream, async (s) => {
  await nextTick();
  if (videoEl.value) videoEl.value.srcObject = s;
});

function endSession() {
  disconnect();
}

const dotClass = computed(() => {
  const s = status.value;
  if (s === "streaming" || s === "connected") return "bg-emerald-400";
  if (s.includes("error") || s.includes("offline") || s === "disconnected") return "bg-rose-400";
  if (s === "idle") return "bg-slate-500";
  return "bg-amber-400 animate-pulse";
});

const phase = computed<"idle" | "connecting" | "live">(() => {
  if (videoStream.value) return "live";
  if (active.value) return "connecting";
  return "idle";
});

const connectingLabel = computed(() => {
  const s = status.value;
  if (s.startsWith("unlocking")) return "Unlocking with the device password…";
  if (s.startsWith("requesting")) return "Waiting for the remote PC to accept…";
  if (s.startsWith("accepted")) return "Accepted — starting the video…";
  if (s === "connecting" || s === "checking" || s === "new") return "Establishing a secure connection…";
  return s.charAt(0).toUpperCase() + s.slice(1);
});

const codeInput = ref("");
function onSubmitCode() {
  submitCode(codeInput.value);
  codeInput.value = "";
}

const chatOpen = ref(false);
const appsOpen = ref(false);
function pickApp(id: string) {
  appsOpen.value = false;
  openApp(id);
}
const draft = ref("");
const chatLog = ref<HTMLDivElement | null>(null);
function toggleChat() {
  chatOpen.value = !chatOpen.value;
  if (chatOpen.value) markChatRead();
}
function sendDraft() {
  const text = draft.value;
  draft.value = "";
  sendChat(text);
}
watch(
  messages,
  () => {
    if (chatOpen.value) markChatRead();
    nextTick(() => {
      if (chatLog.value) chatLog.value.scrollTop = chatLog.value.scrollHeight;
    });
  },
  { deep: true },
);

const isFullscreen = ref(false);
function toggleFullscreen() {
  if (document.fullscreenElement) document.exitFullscreen();
  else screenEl.value?.requestFullscreen?.();
}
onMounted(() => {
  document.addEventListener("fullscreenchange", () => {
    isFullscreen.value = !!document.fullscreenElement;
  });
});

function norm(e: MouseEvent): { x: number; y: number } | null {
  const v = videoEl.value;
  if (!v || !v.videoWidth || !v.videoHeight) return null;
  const rect = v.getBoundingClientRect();
  const scale = Math.min(rect.width / v.videoWidth, rect.height / v.videoHeight);
  const dispW = v.videoWidth * scale;
  const dispH = v.videoHeight * scale;
  const offX = (rect.width - dispW) / 2;
  const offY = (rect.height - dispH) / 2;
  const x = (e.clientX - rect.left - offX) / dispW;
  const y = (e.clientY - rect.top - offY) / dispH;
  if (x < 0 || x > 1 || y < 0 || y > 1) return null;
  return { x, y };
}

function onMouseMove(e: MouseEvent) {
  if (!canControl.value) return;
  const now = performance.now();
  if (now - lastMove < 16) return;
  lastMove = now;
  const p = norm(e);
  if (p) sendInput({ t: "m", x: p.x, y: p.y });
}
function onMouseDown(e: MouseEvent) {
  if (!canControl.value) return;
  screenEl.value?.focus();
  const p = norm(e);
  if (p) sendInput({ t: "m", x: p.x, y: p.y });
  sendInput({ t: "d", b: e.button });
}
function onMouseUp(e: MouseEvent) {
  if (!canControl.value) return;
  sendInput({ t: "u", b: e.button });
}
function onWheel(e: WheelEvent) {
  if (!canControl.value) return;
  e.preventDefault();
  sendInput({ t: "w", dy: e.deltaY });
}
function onContextMenu(e: MouseEvent) {
  if (canControl.value) e.preventDefault();
}
function onKeyDown(e: KeyboardEvent) {
  if (!canControl.value) return;
  e.preventDefault();
  sendInput({ t: "kd", k: e.key });
}
function onKeyUp(e: KeyboardEvent) {
  if (!canControl.value) return;
  e.preventDefault();
  sendInput({ t: "ku", k: e.key });
}
</script>

<template>
  <div class="h-full bg-ink text-slate-200">
    <!-- ════════════════ SESSION (connecting / live) ════════════════ -->
    <div v-if="active" class="flex h-full flex-col">
      <header class="relative z-40 flex h-14 shrink-0 items-center gap-3 border-b border-edge/80 bg-panel/80 px-4 backdrop-blur">
        <div class="flex items-center gap-2.5">
          <span class="grid h-7 w-7 place-items-center rounded-lg bg-gradient-to-br from-accent to-accent2 shadow-[0_2px_10px_-2px] shadow-accent/50">
            <Icon name="monitor" class="h-4 w-4 text-white" />
          </span>
          <div class="leading-none">
            <div class="text-[15px] font-semibold tracking-tight text-slate-100">Arna</div>
            <div class="mt-0.5 text-[10px] font-medium uppercase tracking-[0.15em] text-slate-500">Session</div>
          </div>
        </div>

        <template v-if="phase === 'live'">
          <div class="mx-1 h-6 w-px bg-edge" />
          <div class="flex items-center gap-2">
            <span class="h-2 w-2 rounded-full" :class="dotClass" />
            <span class="font-mono text-sm text-slate-300">{{ connectInfo?.name || agentId }}</span>
          </div>
        </template>

        <div class="ml-auto flex items-center gap-2">
          <span v-if="transferStatus" class="hidden text-xs text-slate-400 sm:inline">{{ transferStatus }}</span>
          <span
            v-if="sessionCode && phase === 'live'"
            class="flex items-center gap-1.5 rounded-full border border-edge bg-ink/60 px-2.5 py-1 font-mono text-xs text-slate-300"
            title="Session code"
          >
            <Icon name="shield" class="h-3.5 w-3.5 text-slate-500" />{{ sessionCode }}
          </span>
          <span v-if="canControl" class="flex items-center gap-1.5 rounded-full bg-accent/15 px-2.5 py-1 text-xs font-semibold text-accent2">
            <Icon name="keyboard" class="h-3.5 w-3.5" /> control
          </span>

          <template v-if="phase === 'live'">
            <!-- In a bubble: a chip + exit -->
            <button
              v-if="bubbleApp"
              class="flex items-center gap-1.5 rounded-lg border border-accent/50 bg-accent/15 px-2.5 py-1.5 text-xs font-semibold text-accent2 transition hover:bg-accent/25"
              title="Close the app and go back to the screen"
              @click="exitBubble"
            >
              <Icon name="layout" class="h-3.5 w-3.5" />
              {{ apps.find((a) => a.id === bubbleApp)?.label || "App" }}
              <Icon name="x" class="h-3.5 w-3.5" />
            </button>

            <!-- Apps menu (open one app in a sandbox bubble) -->
            <div v-else-if="apps.length" class="relative">
              <button
                class="flex items-center gap-1.5 rounded-lg border border-edge bg-ink/50 px-2.5 py-1.5 text-xs font-medium text-slate-200 transition hover:border-slate-600 hover:bg-ink focus-visible:ring-2 focus-visible:ring-accent"
                title="Open one app in a sandbox (you keep working)"
                @click="appsOpen = !appsOpen"
              >
                <Icon name="layout" class="h-4 w-4" /> Apps
              </button>
              <div
                v-if="appsOpen"
                class="absolute right-0 top-full z-40 mt-1.5 w-52 overflow-hidden rounded-xl border border-edge bg-panel py-1 shadow-2xl shadow-black/50"
              >
                <p class="px-3 pb-1 pt-1.5 text-[10px] font-semibold uppercase tracking-wider text-slate-600">Open in a sandbox</p>
                <button
                  v-for="a in apps"
                  :key="a.id"
                  class="flex w-full items-center gap-2.5 px-3 py-2 text-left text-sm text-slate-200 transition hover:bg-ink"
                  @click="pickApp(a.id)"
                >
                  <Icon name="layout" class="h-4 w-4 text-slate-500" /> {{ a.label }}
                </button>
              </div>
            </div>

            <div
              v-if="monitors.length > 1 && !bubbleApp"
              class="flex items-center gap-0.5 rounded-lg border border-edge bg-ink/50 p-0.5"
              title="Choose which screen to view"
            >
              <Icon name="monitor" class="ml-1 mr-0.5 h-3.5 w-3.5 text-slate-500" />
              <button
                v-for="m in monitors"
                :key="m.index"
                class="rounded-md px-2 py-1 text-xs font-semibold transition"
                :class="m.index === currentMonitor ? 'bg-accent text-white' : 'text-slate-300 hover:bg-ink hover:text-slate-100'"
                :title="`${m.label} — ${m.width}×${m.height}${m.primary ? ' (primary)' : ''}`"
                @click="selectMonitor(m.index)"
              >
                {{ m.index + 1 }}
              </button>
            </div>
            <button
              v-if="canChat"
              class="relative grid h-8 w-8 place-items-center rounded-lg border bg-ink/50 transition focus-visible:ring-2 focus-visible:ring-accent"
              :class="chatOpen ? 'border-accent text-accent2' : 'border-edge text-slate-300 hover:border-slate-600 hover:text-slate-100'"
              title="Chat"
              @click="toggleChat"
            >
              <Icon name="chat" class="h-4 w-4" />
              <span v-if="unread > 0" class="absolute -right-1 -top-1 grid h-4 min-w-4 place-items-center rounded-full bg-accent px-1 text-[10px] font-bold text-white">
                {{ unread > 9 ? "9+" : unread }}
              </span>
            </button>
            <button
              class="grid h-8 w-8 place-items-center rounded-lg border border-edge bg-ink/50 text-slate-300 transition hover:border-slate-600 hover:text-slate-100 focus-visible:ring-2 focus-visible:ring-accent"
              :title="isFullscreen ? 'Exit fullscreen' : 'Fullscreen'"
              @click="toggleFullscreen"
            >
              <Icon :name="isFullscreen ? 'minimize' : 'maximize'" class="h-4 w-4" />
            </button>
            <button
              v-if="canSendFiles"
              class="grid h-8 w-8 place-items-center rounded-lg border border-edge bg-ink/50 text-slate-300 transition hover:border-slate-600 hover:text-slate-100 focus-visible:ring-2 focus-visible:ring-accent"
              title="Download a file from the remote PC"
              @click="requestDownload"
            >
              <Icon name="download" class="h-4 w-4" />
            </button>
            <button
              v-if="canSendFiles"
              class="flex items-center gap-1.5 rounded-lg border border-edge bg-ink/50 px-3 py-1.5 text-sm font-medium text-slate-200 transition hover:border-slate-600 hover:bg-ink focus-visible:ring-2 focus-visible:ring-accent"
              @click="pickFile"
            >
              <Icon name="upload" class="h-4 w-4" /> Send file
            </button>
          </template>
          <button
            class="flex items-center gap-1.5 rounded-lg bg-rose-500/90 px-3 py-1.5 text-sm font-semibold text-white transition hover:bg-rose-500 focus-visible:ring-2 focus-visible:ring-rose-400"
            @click="endSession"
          >
            <Icon name="x" class="h-4 w-4" /> End
          </button>
        </div>
        <input ref="fileInput" type="file" class="hidden" @change="onFileChosen" />
      </header>

      <main
        ref="screenEl"
        tabindex="0"
        class="relative grid flex-1 place-items-center overflow-hidden bg-ink outline-none"
        :class="{ 'cursor-none': canControl && videoStream, 'stage-grid': phase !== 'live' }"
        @keydown="onKeyDown"
        @keyup="onKeyUp"
        @focus="pushClipboard"
        @pointerdown="pushClipboard"
        @contextmenu="onContextMenu"
        @dragover.prevent="onDragOver"
        @dragleave="dragOver = false"
        @drop.prevent="onDrop"
      >
        <div v-if="transferProgress > 0 && transferProgress < 1" class="pointer-events-none absolute inset-x-0 top-0 z-30 h-0.5 bg-edge">
          <div class="h-full bg-accent transition-[width] duration-150" :style="{ width: transferProgress * 100 + '%' }" />
        </div>

        <div v-if="dragOver && canSendFiles" class="pointer-events-none absolute inset-4 z-30 grid place-items-center rounded-2xl border-2 border-dashed border-accent/70 bg-ink/80 backdrop-blur-sm">
          <div class="text-center">
            <Icon name="upload" class="mx-auto mb-2 h-8 w-8 text-accent2" />
            <p class="font-medium text-slate-100">Drop to send to the remote PC</p>
          </div>
        </div>

        <!-- LIVE -->
        <template v-if="phase === 'live'">
          <video
            ref="videoEl"
            autoplay
            playsinline
            muted
            class="max-h-full max-w-full object-contain"
            @mousemove="onMouseMove"
            @mousedown="onMouseDown"
            @mouseup="onMouseUp"
            @wheel="onWheel"
          />
          <div class="pointer-events-none absolute right-4 top-4 z-20 flex items-center gap-2 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-3 py-1 text-xs font-semibold text-emerald-300 backdrop-blur">
            <span class="h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-400" /> LIVE
          </div>
          <div v-if="canControl" class="pointer-events-none absolute bottom-4 left-1/2 z-20 -translate-x-1/2 rounded-full border border-edge/80 bg-panel/80 px-3.5 py-1.5 text-xs text-slate-400 backdrop-blur">
            Click to control · keystrokes are forwarded
          </div>

          <transition name="slide">
            <aside v-if="chatOpen" class="absolute bottom-0 right-0 top-0 z-30 flex w-80 flex-col border-l border-edge bg-panel/95 backdrop-blur">
              <header class="flex items-center gap-2 border-b border-edge px-4 py-3">
                <Icon name="chat" class="h-4 w-4 text-accent2" />
                <span class="text-sm font-semibold text-slate-100">Chat</span>
                <button class="ml-auto grid h-6 w-6 place-items-center rounded-md text-slate-500 transition hover:bg-ink hover:text-slate-200" title="Close" @click="toggleChat">
                  <Icon name="x" class="h-4 w-4" />
                </button>
              </header>
              <div ref="chatLog" class="flex-1 space-y-2 overflow-y-auto px-3 py-3">
                <p v-if="!messages.length" class="mt-6 px-3 text-center text-sm text-slate-600">No messages yet. Say hello to the person at the remote PC.</p>
                <div v-for="(m, i) in messages" :key="i" class="flex" :class="m.mine ? 'justify-end' : 'justify-start'">
                  <span class="max-w-[85%] whitespace-pre-wrap break-words rounded-2xl px-3 py-1.5 text-sm" :class="m.mine ? 'rounded-br-sm bg-accent text-white' : 'rounded-bl-sm bg-ink text-slate-200'">{{ m.text }}</span>
                </div>
              </div>
              <form class="flex items-center gap-2 border-t border-edge p-3" @submit.prevent="sendDraft">
                <input
                  v-model="draft"
                  placeholder="Type a message…"
                  autocomplete="off"
                  @keydown.enter.prevent="sendDraft"
                  class="min-w-0 flex-1 rounded-lg border border-edge bg-ink px-3 py-2 text-sm text-slate-100 outline-none transition placeholder:text-slate-600 focus:border-accent focus:ring-2 focus:ring-accent/30"
                />
                <button type="submit" :disabled="!draft.trim()" class="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-accent text-white transition hover:bg-accent/90 disabled:opacity-40 focus-visible:ring-2 focus-visible:ring-accent">
                  <Icon name="send" class="h-4 w-4" />
                </button>
              </form>
            </aside>
          </transition>
        </template>

        <!-- CONNECTING -->
        <div v-else class="flex flex-col items-center gap-5 px-6 text-center">
          <template v-if="awaitingCode">
            <div class="grid h-14 w-14 place-items-center rounded-2xl bg-accent/10 text-accent2 ring-1 ring-inset ring-accent/20">
              <Icon name="shield" class="h-6 w-6" />
            </div>
            <div>
              <p class="text-base font-medium text-slate-100">Enter the code</p>
              <p class="mt-1 max-w-xs text-sm text-slate-500">
                The person at <span class="font-mono text-slate-400">{{ agentId }}</span> will read you a 6-digit code.
              </p>
            </div>
            <form class="flex flex-col items-center gap-2" @submit.prevent="onSubmitCode">
              <input
                v-model="codeInput"
                inputmode="numeric"
                maxlength="6"
                placeholder="000000"
                class="w-44 rounded-lg border border-edge bg-ink px-3 py-2.5 text-center font-mono text-2xl tracking-[0.3em] text-slate-100 outline-none focus:border-accent focus:ring-2 focus:ring-accent/30"
              />
              <p v-if="codeError" class="text-xs text-rose-300">{{ codeError }}</p>
              <div class="mt-1 flex gap-2">
                <button type="button" class="rounded-lg border border-edge px-4 py-2 text-sm font-medium text-slate-300 transition hover:bg-panel" @click="endSession">Cancel</button>
                <button type="submit" :disabled="codeInput.trim().length < 6" class="rounded-lg bg-accent px-5 py-2 text-sm font-semibold text-white transition hover:bg-accent/90 disabled:opacity-40">Connect</button>
              </div>
            </form>
          </template>

          <template v-else>
            <div class="relative grid h-16 w-16 place-items-center">
              <span class="absolute inset-0 animate-spin rounded-full border-2 border-edge border-t-accent" />
              <Icon name="monitor" class="h-6 w-6 text-slate-400" />
            </div>
            <div>
              <p class="text-base font-medium text-slate-100">{{ connectingLabel }}</p>
              <p class="mt-1 font-mono text-sm text-slate-500">{{ connectInfo?.name || agentId }}</p>
            </div>
            <p v-if="sessionCode" class="rounded-lg border border-edge bg-panel px-3 py-2 text-sm text-slate-400">
              If asked for a code, share <span class="font-mono font-semibold text-accent2">{{ sessionCode }}</span>
            </p>
            <button class="rounded-lg border border-edge px-4 py-2 text-sm font-medium text-slate-300 transition hover:bg-panel focus-visible:ring-2 focus-visible:ring-accent" @click="endSession">Cancel</button>
          </template>
        </div>
      </main>
    </div>

    <!-- ════════════════ HOME (signed in) ════════════════ -->
    <div v-else-if="loggedIn" class="flex h-full">
      <!-- Sidebar -->
      <aside
        class="flex shrink-0 flex-col overflow-hidden border-r border-edge/80 bg-panel/60 transition-[width] duration-200"
        :class="sidebarOpen ? 'w-60' : 'w-0'"
      >
        <div class="flex h-14 items-center gap-2.5 px-4">
          <span class="grid h-8 w-8 place-items-center rounded-lg bg-gradient-to-br from-accent to-accent2 shadow-[0_2px_10px_-2px] shadow-accent/50">
            <Icon name="monitor" class="h-4 w-4 text-white" />
          </span>
          <div class="leading-none">
            <div class="text-[15px] font-semibold tracking-tight text-slate-100">Arna</div>
            <div class="mt-0.5 text-[10px] font-medium uppercase tracking-[0.15em] text-slate-500">Remote</div>
          </div>
        </div>

        <nav class="mt-3 flex flex-1 flex-col gap-1 px-3">
          <button
            class="flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition"
            :class="nav === 'devices' ? 'bg-accent/15 text-accent2' : 'text-slate-400 hover:bg-ink hover:text-slate-200'"
            @click="go('devices')"
          >
            <Icon name="layout" class="h-4 w-4" /> Dashboard
          </button>
          <button
            class="flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition"
            :class="nav === 'profile' ? 'bg-accent/15 text-accent2' : 'text-slate-400 hover:bg-ink hover:text-slate-200'"
            @click="go('profile')"
          >
            <Icon name="user" class="h-4 w-4" /> Profile
          </button>
        </nav>

        <div class="border-t border-edge/80 p-3">
          <div class="flex items-center gap-2.5 rounded-lg px-2 py-1.5">
            <span class="grid h-8 w-8 shrink-0 place-items-center rounded-full bg-accent/15 text-xs font-bold text-accent2">
              {{ (accountEmail || "?").charAt(0).toUpperCase() }}
            </span>
            <div class="min-w-0 flex-1">
              <div class="truncate text-xs font-medium text-slate-300">{{ accountEmail }}</div>
              <div class="truncate font-mono text-[11px] text-slate-500">ID {{ userId || "…" }}</div>
            </div>
            <button class="grid h-7 w-7 shrink-0 place-items-center rounded-md text-slate-500 transition hover:bg-ink hover:text-slate-200" title="Sign out" @click="authLogout()">
              <Icon name="logout" class="h-4 w-4" />
            </button>
          </div>
        </div>
      </aside>

      <!-- Content -->
      <div class="flex min-w-0 flex-1 flex-col">
        <header class="flex h-14 shrink-0 items-center gap-3 border-b border-edge/80 bg-panel/40 px-4 backdrop-blur">
          <button class="grid h-9 w-9 place-items-center rounded-lg text-slate-400 transition hover:bg-ink hover:text-slate-200" title="Toggle menu" @click="sidebarOpen = !sidebarOpen">
            <Icon name="menu" class="h-5 w-5" />
          </button>
          <h1 class="text-base font-semibold tracking-tight text-slate-100">{{ nav === "devices" ? "Dashboard" : "Profile" }}</h1>
        </header>

        <main class="flex-1 overflow-y-auto p-6">
          <!-- ── DASHBOARD ── -->
          <div v-if="nav === 'devices'" class="mx-auto max-w-5xl space-y-6">
            <div v-if="errorMessage" role="alert" class="flex items-start gap-2.5 rounded-xl border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-200">
              <Icon :name="errorKind === 'offline' ? 'offline' : 'alert'" class="mt-0.5 h-4 w-4 shrink-0 text-rose-400" />
              <span>{{ errorMessage }}</span>
            </div>

            <!-- Quick connect module -->
            <section class="rounded-2xl border border-edge bg-gradient-to-br from-panel to-panel/40 p-6 shadow-xl shadow-black/20">
              <div class="flex items-center gap-2.5">
                <span class="grid h-9 w-9 place-items-center rounded-xl bg-accent/15 text-accent2"><Icon name="power" class="h-4.5 w-4.5" /></span>
                <div>
                  <h2 class="text-sm font-semibold text-slate-100">Connect to a device</h2>
                  <p class="text-xs text-slate-500">Enter its ID — we'll look up the name before connecting.</p>
                </div>
              </div>
              <form class="mt-4 flex flex-col gap-2 sm:flex-row" @submit.prevent="openConnect(connectId)">
                <input
                  v-model="connectId"
                  placeholder="device id, e.g. front-desk-pc"
                  autocomplete="off"
                  spellcheck="false"
                  class="min-w-0 flex-1 rounded-lg border border-edge bg-ink px-3.5 py-2.5 font-mono text-sm text-slate-100 outline-none transition placeholder:text-slate-600 focus:border-accent focus:ring-2 focus:ring-accent/30"
                />
                <button type="submit" :disabled="!connectId.trim()" class="flex items-center justify-center gap-2 rounded-lg bg-accent px-5 py-2.5 text-sm font-semibold text-white shadow-lg shadow-accent/25 transition hover:bg-accent/90 disabled:opacity-40">
                  <Icon name="arrowRight" class="h-4 w-4" /> Connect
                </button>
              </form>
            </section>

            <!-- Your devices module -->
            <section>
              <div class="mb-3 flex items-center justify-between">
                <h2 class="text-sm font-semibold uppercase tracking-wider text-slate-500">Your devices</h2>
                <button class="flex items-center gap-1.5 rounded-lg border border-edge px-3 py-1.5 text-xs font-medium text-slate-200 transition hover:bg-panel" @click="showAddDevice = true">
                  <Icon name="plus" class="h-3.5 w-3.5" /> Add a device
                </button>
              </div>

              <div v-if="myDevices.length" class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                <div
                  v-for="d in myDevices"
                  :key="d.id"
                  class="group flex flex-col rounded-2xl border border-edge bg-panel/70 p-4 transition hover:border-accent/60 hover:bg-panel"
                >
                  <div class="flex items-start gap-3">
                    <span class="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-ink text-slate-400 group-hover:text-accent2"><Icon name="monitor" class="h-5 w-5" /></span>
                    <div class="min-w-0 flex-1">
                      <div class="truncate text-sm font-semibold text-slate-100">{{ d.name }}</div>
                      <div class="truncate font-mono text-xs text-slate-500">{{ d.id }}</div>
                    </div>
                    <span
                      v-if="d.has_password"
                      class="flex items-center gap-1 rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-semibold text-emerald-300"
                      title="Unattended access password is set"
                    >
                      <Icon name="lock" class="h-3 w-3" /> PW
                    </span>
                  </div>
                  <div class="mt-4 flex gap-2">
                    <button class="flex flex-1 items-center justify-center gap-1.5 rounded-lg bg-accent px-3 py-2 text-xs font-semibold text-white transition hover:bg-accent/90" @click="openConnect(d.id, d)">
                      <Icon name="arrowRight" class="h-3.5 w-3.5" /> Connect
                    </button>
                    <button class="grid h-8 w-8 place-items-center rounded-lg border border-edge text-slate-400 transition hover:bg-ink hover:text-slate-200" :title="d.has_password ? 'Change access password' : 'Set access password'" @click="openPw(d)">
                      <Icon name="lock" class="h-4 w-4" />
                    </button>
                  </div>
                </div>
              </div>
              <div v-else class="rounded-2xl border border-dashed border-edge px-4 py-10 text-center">
                <Icon name="monitor" class="mx-auto h-8 w-8 text-slate-600" />
                <p class="mt-3 text-sm text-slate-400">No devices yet.</p>
                <p class="mt-1 text-xs text-slate-600">Add one, then run Arna on that PC and pair it to bring it online.</p>
              </div>
            </section>
          </div>

          <!-- ── PROFILE ── -->
          <div v-else class="mx-auto max-w-3xl space-y-6">
            <section class="rounded-2xl border border-edge bg-gradient-to-br from-panel to-panel/40 p-6 shadow-xl shadow-black/20">
              <div class="flex flex-col items-center gap-5 sm:flex-row sm:items-start">
                <div class="rounded-xl bg-white p-3 shadow-lg">
                  <QrCode :value="connectLink" :size="148" />
                </div>
                <div class="min-w-0 flex-1 text-center sm:text-left">
                  <p class="text-xs font-semibold uppercase tracking-wider text-slate-500">Your Arna ID</p>
                  <div class="mt-1 flex items-center justify-center gap-2 sm:justify-start">
                    <span class="font-mono text-3xl font-bold tracking-wider text-slate-100">{{ userId || "…" }}</span>
                    <button class="grid h-8 w-8 place-items-center rounded-lg border border-edge text-slate-400 transition hover:bg-ink hover:text-slate-200" title="Copy ID" @click="copyText(userId || '', 'id')">
                      <Icon :name="copied === 'id' ? 'check' : 'copy'" class="h-4 w-4" :class="copied === 'id' ? 'text-emerald-400' : ''" />
                    </button>
                  </div>
                  <p class="mt-1 text-sm text-slate-500">{{ accountEmail }}</p>
                  <p class="mt-3 text-xs text-slate-500">Share your ID or let someone scan the code to reach you.</p>
                </div>
              </div>
            </section>

            <section class="space-y-3">
              <div class="rounded-xl border border-edge bg-panel/60 p-4">
                <div class="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-slate-500">
                  <Icon name="link" class="h-3.5 w-3.5" /> Connect link
                </div>
                <div class="mt-2 flex items-center gap-2">
                  <span class="min-w-0 flex-1 truncate rounded-lg border border-edge bg-ink px-3 py-2 font-mono text-xs text-slate-300">{{ connectLink }}</span>
                  <button class="grid h-9 w-9 shrink-0 place-items-center rounded-lg border border-edge text-slate-400 transition hover:bg-ink hover:text-slate-200" title="Copy" @click="copyText(connectLink, 'connect')">
                    <Icon :name="copied === 'connect' ? 'check' : 'copy'" class="h-4 w-4" :class="copied === 'connect' ? 'text-emerald-400' : ''" />
                  </button>
                </div>
              </div>

              <div class="rounded-xl border border-edge bg-panel/60 p-4">
                <div class="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-slate-500">
                  <Icon name="user" class="h-3.5 w-3.5" /> Invite link
                </div>
                <div class="mt-2 flex items-center gap-2">
                  <span class="min-w-0 flex-1 truncate rounded-lg border border-edge bg-ink px-3 py-2 font-mono text-xs text-slate-300">{{ inviteLink }}</span>
                  <button class="grid h-9 w-9 shrink-0 place-items-center rounded-lg border border-edge text-slate-400 transition hover:bg-ink hover:text-slate-200" title="Copy" @click="copyText(inviteLink, 'invite')">
                    <Icon :name="copied === 'invite' ? 'check' : 'copy'" class="h-4 w-4" :class="copied === 'invite' ? 'text-emerald-400' : ''" />
                  </button>
                </div>
                <p class="mt-2 text-xs text-slate-600">Send this to invite someone to create an Arna account.</p>
              </div>

              <div class="rounded-xl border border-edge bg-panel/60 p-4">
                <label class="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-slate-500">
                  <Icon name="shield" class="h-3.5 w-3.5" /> Server
                </label>
                <input v-model="backend" class="mt-2 w-full rounded-lg border border-edge bg-ink px-3 py-2 font-mono text-xs text-slate-300 outline-none transition focus:border-accent focus:ring-2 focus:ring-accent/30" />
                <p class="mt-2 text-xs text-slate-600">The Arna server this app talks to (self-host friendly).</p>
              </div>
            </section>
          </div>
        </main>
      </div>
    </div>

    <!-- ════════════════ SIGNED OUT (auth) ════════════════ -->
    <div v-else class="stage-grid grid h-full place-items-center px-6">
      <div class="w-full max-w-md">
        <!-- Manual connect (self-host, no account) -->
        <template v-if="showManual">
          <div class="rounded-2xl border border-edge bg-panel/90 p-7 shadow-2xl shadow-black/40">
            <div class="mb-5 flex items-center gap-3">
              <span class="grid h-11 w-11 place-items-center rounded-xl bg-accent/10 text-accent2 ring-1 ring-inset ring-accent/20"><Icon name="monitor" class="h-5 w-5" /></span>
              <div>
                <h1 class="text-lg font-semibold tracking-tight text-slate-100">Connect manually</h1>
                <p class="text-sm text-slate-500">Enter a server + device ID (no account).</p>
              </div>
            </div>
            <div v-if="errorMessage" role="alert" class="mb-4 flex items-start gap-2.5 rounded-lg border border-rose-500/30 bg-rose-500/10 px-3 py-2.5 text-sm text-rose-200">
              <Icon :name="errorKind === 'offline' ? 'offline' : 'alert'" class="mt-0.5 h-4 w-4 shrink-0 text-rose-400" />
              <span>{{ errorMessage }}</span>
            </div>
            <form class="space-y-3.5" @submit.prevent="manualConnect">
              <div>
                <label class="mb-1 block text-xs font-medium text-slate-400">Device ID</label>
                <input v-model="agentId" placeholder="agent-1" autocomplete="off" spellcheck="false" class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 text-slate-100 outline-none transition placeholder:text-slate-600 focus:border-accent focus:ring-2 focus:ring-accent/30" />
              </div>
              <div>
                <label class="mb-1 block text-xs font-medium text-slate-400">Signaling server</label>
                <input v-model="backend" class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 font-mono text-sm text-slate-300 outline-none transition focus:border-accent focus:ring-2 focus:ring-accent/30" />
              </div>
              <div>
                <label class="mb-1 block text-xs font-medium text-slate-400">SSO ticket <span class="text-slate-600">· optional</span></label>
                <input v-model="ticket" placeholder="paste a signed ticket if required" class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 text-sm text-slate-300 outline-none transition placeholder:text-slate-600 focus:border-accent focus:ring-2 focus:ring-accent/30" />
              </div>
              <button type="submit" class="mt-1 flex w-full items-center justify-center gap-2 rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-white shadow-lg shadow-accent/25 transition hover:bg-accent/90">
                <Icon name="power" class="h-4 w-4" /> Connect
              </button>
            </form>
          </div>
          <p class="mt-4 text-center text-xs">
            <button class="text-slate-500 underline-offset-2 hover:text-slate-300 hover:underline" @click="showManual = false">← Back to sign in</button>
          </p>
        </template>

        <!-- Login / signup -->
        <template v-else>
          <div class="rounded-2xl border border-edge bg-panel/90 p-7 shadow-2xl shadow-black/40">
            <div class="mb-5 flex items-center gap-3">
              <span class="grid h-11 w-11 place-items-center rounded-xl bg-gradient-to-br from-accent to-accent2 shadow-lg shadow-accent/25"><Icon name="monitor" class="h-5 w-5 text-white" /></span>
              <div>
                <h1 class="text-lg font-semibold tracking-tight text-slate-100">{{ isSignup ? "Create your account" : "Welcome to Arna" }}</h1>
                <p class="text-sm text-slate-500">{{ isSignup ? "Sign up to manage your devices." : "Sign in to reach your devices." }}</p>
              </div>
            </div>
            <div v-if="authError" role="alert" class="mb-4 flex items-start gap-2.5 rounded-lg border border-rose-500/30 bg-rose-500/10 px-3 py-2.5 text-sm text-rose-200">
              <Icon name="alert" class="mt-0.5 h-4 w-4 shrink-0 text-rose-400" />
              <span>{{ authError }}</span>
            </div>
            <form class="space-y-3.5" @submit.prevent="submitAuth">
              <div>
                <label class="mb-1 block text-xs font-medium text-slate-400">Email</label>
                <input v-model="loginEmail" type="email" autocomplete="email" placeholder="you@example.com" class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 text-slate-100 outline-none transition placeholder:text-slate-600 focus:border-accent focus:ring-2 focus:ring-accent/30" />
              </div>
              <div>
                <label class="mb-1 block text-xs font-medium text-slate-400">Password</label>
                <input v-model="loginPassword" type="password" :autocomplete="isSignup ? 'new-password' : 'current-password'" placeholder="••••••••" class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 text-slate-100 outline-none transition placeholder:text-slate-600 focus:border-accent focus:ring-2 focus:ring-accent/30" />
              </div>
              <button type="submit" :disabled="authBusy" class="mt-1 w-full rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-white shadow-lg shadow-accent/25 transition hover:bg-accent/90 disabled:opacity-50">
                {{ authBusy ? "…" : isSignup ? "Create account" : "Sign in" }}
              </button>
            </form>
            <p class="mt-4 text-center text-sm text-slate-500">
              {{ isSignup ? "Already have an account?" : "New here?" }}
              <button class="font-medium text-accent2 hover:underline" @click="isSignup = !isSignup">{{ isSignup ? "Sign in" : "Create one" }}</button>
            </p>

            <!-- Server (point at a LAN / self-hosted / VPS backend) -->
            <div class="mt-5 border-t border-edge/70 pt-4">
              <button class="flex w-full items-center gap-1.5 text-xs font-medium text-slate-500 transition hover:text-slate-300" @click="showServer = !showServer">
                <Icon name="shield" class="h-3.5 w-3.5" /> Server
                <span class="ml-auto font-mono text-[11px] text-slate-600">{{ showServer ? "▾" : "▸" }}</span>
              </button>
              <input
                v-if="showServer"
                v-model="backend"
                spellcheck="false"
                class="mt-2 w-full rounded-lg border border-edge bg-ink px-3 py-2 font-mono text-xs text-slate-300 outline-none transition focus:border-accent focus:ring-2 focus:ring-accent/30"
              />
            </div>
          </div>
          <p class="mt-4 text-center text-xs">
            <button class="text-slate-500 underline-offset-2 hover:text-slate-300 hover:underline" @click="showManual = true">Connect without an account</button>
          </p>
        </template>
      </div>
    </div>

    <!-- ════════════════ Connect dialog ════════════════ -->
    <div v-if="showConnectDialog" class="fixed inset-0 z-50 grid place-items-center bg-black/60 p-6" @click.self="showConnectDialog = false">
      <div class="w-full max-w-sm rounded-2xl border border-edge bg-panel p-6 shadow-2xl">
        <div class="flex items-center gap-3">
          <span class="grid h-11 w-11 shrink-0 place-items-center rounded-xl bg-accent/10 text-accent2 ring-1 ring-inset ring-accent/20"><Icon name="monitor" class="h-5 w-5" /></span>
          <div class="min-w-0">
            <h2 class="truncate text-base font-semibold text-slate-100">
              {{ connectResolving ? "Looking up…" : connectInfo?.name || connectId }}
            </h2>
            <p class="truncate font-mono text-xs text-slate-500">{{ connectId }}</p>
          </div>
        </div>

        <div v-if="connectError" class="mt-4 flex items-start gap-2 rounded-lg border border-rose-500/30 bg-rose-500/10 px-3 py-2 text-sm text-rose-200">
          <Icon name="alert" class="mt-0.5 h-4 w-4 shrink-0 text-rose-400" /><span>{{ connectError }}</span>
        </div>

        <!-- Password (unattended access) -->
        <template v-if="connectInfo?.has_password">
          <form class="mt-5" @submit.prevent="doConnect(true)">
            <label class="mb-1 flex items-center gap-1.5 text-xs font-medium text-slate-400"><Icon name="lock" class="h-3.5 w-3.5" /> Device password</label>
            <input
              v-model="connectPassword"
              type="password"
              placeholder="enter to connect instantly"
              class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 text-slate-100 outline-none transition placeholder:text-slate-600 focus:border-accent focus:ring-2 focus:ring-accent/30"
            />
            <button type="submit" :disabled="!connectPassword" class="mt-3 flex w-full items-center justify-center gap-2 rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-white transition hover:bg-accent/90 disabled:opacity-40">
              <Icon name="power" class="h-4 w-4" /> Connect with password
            </button>
          </form>
          <div class="my-3 flex items-center gap-3 text-[11px] uppercase tracking-wider text-slate-600">
            <span class="h-px flex-1 bg-edge" /> or <span class="h-px flex-1 bg-edge" />
          </div>
          <button class="flex w-full items-center justify-center gap-2 rounded-lg border border-edge px-4 py-2.5 text-sm font-medium text-slate-200 transition hover:bg-ink" @click="doConnect(false)">
            Ask the operator to accept
          </button>
        </template>

        <!-- No password set: just request, operator accepts -->
        <template v-else>
          <p class="mt-4 text-sm text-slate-400">The person at this PC will get a request and must <span class="text-slate-200">Accept</span> before you connect.</p>
          <button class="mt-4 flex w-full items-center justify-center gap-2 rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-white transition hover:bg-accent/90" @click="doConnect(false)">
            <Icon name="arrowRight" class="h-4 w-4" /> Ask to connect
          </button>
        </template>

        <button class="mt-3 w-full rounded-lg px-4 py-2 text-sm text-slate-500 transition hover:text-slate-300" @click="showConnectDialog = false">Cancel</button>
      </div>
    </div>

    <!-- ════════════════ Set device password ════════════════ -->
    <div v-if="pwDevice" class="fixed inset-0 z-50 grid place-items-center bg-black/60 p-6" @click.self="pwDevice = null">
      <div class="w-full max-w-sm rounded-2xl border border-edge bg-panel p-6 shadow-2xl">
        <h2 class="flex items-center gap-2 text-base font-semibold text-slate-100"><Icon name="lock" class="h-4 w-4 text-accent2" /> Access password</h2>
        <p class="mt-1 text-sm text-slate-500">
          For <span class="font-medium text-slate-300">{{ pwDevice.name }}</span>. Anyone with this password connects without the operator accepting.
        </p>
        <form class="mt-4" @submit.prevent="savePw(false)">
          <input
            v-model="pwInput"
            type="password"
            placeholder="new password (4+ characters)"
            class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 text-slate-100 outline-none transition placeholder:text-slate-600 focus:border-accent focus:ring-2 focus:ring-accent/30"
          />
          <p v-if="pwError" class="mt-2 text-xs text-rose-300">{{ pwError }}</p>
          <div class="mt-4 flex items-center justify-between gap-2">
            <button v-if="pwDevice.has_password" type="button" :disabled="pwBusy" class="rounded-lg border border-rose-500/40 px-3 py-2 text-xs font-medium text-rose-300 transition hover:bg-rose-500/10 disabled:opacity-40" @click="savePw(true)">
              Remove password
            </button>
            <span v-else />
            <div class="flex gap-2">
              <button type="button" class="rounded-lg border border-edge px-4 py-2 text-sm font-medium text-slate-300 transition hover:bg-ink" @click="pwDevice = null">Cancel</button>
              <button type="submit" :disabled="pwInput.length < 4 || pwBusy" class="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-white transition hover:bg-accent/90 disabled:opacity-40">Save</button>
            </div>
          </div>
        </form>
      </div>
    </div>

    <!-- ════════════════ Add-device modal ════════════════ -->
    <div v-if="showAddDevice" class="fixed inset-0 z-50 grid place-items-center bg-black/60 p-6" @click.self="resetAddDevice">
      <div class="w-full max-w-md rounded-2xl border border-edge bg-panel p-7 shadow-2xl">
        <h2 class="text-lg font-semibold text-slate-100">Add a device</h2>
        <template v-if="!newDeviceToken">
          <p class="mt-1 text-sm text-slate-500">Give it an id and a friendly name. You'll get a token to set up the device.</p>
          <form class="mt-5 space-y-3.5" @submit.prevent="submitAddDevice">
            <div>
              <label class="mb-1 block text-xs font-medium text-slate-400">Device id</label>
              <input v-model="newDeviceId" placeholder="front-desk-pc" autocomplete="off" spellcheck="false" class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 font-mono text-sm text-slate-100 outline-none focus:border-accent focus:ring-2 focus:ring-accent/30" />
            </div>
            <div>
              <label class="mb-1 block text-xs font-medium text-slate-400">Name</label>
              <input v-model="newDeviceName" placeholder="Front desk PC" class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 text-sm text-slate-100 outline-none focus:border-accent focus:ring-2 focus:ring-accent/30" />
            </div>
            <div v-if="authError" class="text-xs text-rose-300">{{ authError }}</div>
            <div class="flex justify-end gap-2 pt-1">
              <button type="button" class="rounded-lg border border-edge px-4 py-2 text-sm font-medium text-slate-300 hover:bg-ink" @click="resetAddDevice">Cancel</button>
              <button type="submit" :disabled="!newDeviceId.trim()" class="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-white hover:bg-accent/90 disabled:opacity-40">Add device</button>
            </div>
          </form>
        </template>
        <template v-else>
          <p class="mt-1 text-sm text-slate-400">Device added. On <span class="font-mono text-slate-200">{{ newDeviceId }}</span>, open Arna there and pair with these:</p>
          <div class="mt-4 space-y-2">
            <div class="rounded-lg border border-edge bg-ink px-3 py-2 font-mono text-xs text-slate-300">
              Device ID: <span class="text-accent2">{{ newDeviceId }}</span>
            </div>
            <div class="flex items-center gap-2 rounded-lg border border-edge bg-ink px-3 py-2">
              <span class="min-w-0 flex-1 truncate font-mono text-xs text-slate-300">Token: {{ newDeviceToken }}</span>
              <button class="grid h-7 w-7 shrink-0 place-items-center rounded-md border border-edge text-slate-400 hover:text-slate-100" title="Copy token" @click="copyText(newDeviceToken, 'tok')">
                <Icon :name="copied === 'tok' ? 'check' : 'copy'" class="h-3.5 w-3.5" :class="copied === 'tok' ? 'text-emerald-400' : ''" />
              </button>
            </div>
          </div>
          <p class="mt-3 text-xs text-slate-500">Keep this token secret — it lets that PC come online as your device.</p>
          <div class="mt-5 flex justify-end">
            <button class="rounded-lg bg-accent px-5 py-2 text-sm font-semibold text-white hover:bg-accent/90" @click="resetAddDevice">Done</button>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Chat panel slide-in. */
.slide-enter-active,
.slide-leave-active {
  transition: transform 0.2s ease, opacity 0.2s ease;
}
.slide-enter-from,
.slide-leave-to {
  transform: translateX(100%);
  opacity: 0;
}

/* Subtle dotted grid behind the connection / connecting states. */
.stage-grid {
  background-image: radial-gradient(circle at center, rgba(109, 94, 252, 0.07), transparent 60%),
    radial-gradient(rgba(255, 255, 255, 0.035) 1px, transparent 1px);
  background-size: 100% 100%, 22px 22px;
}
</style>
