<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useRemote } from "./composables/useRemote";
import { useAuth } from "./composables/useAuth";
import Icon from "./components/Icon.vue";

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
/** Recently-used agents other than the one currently in the field. */
const recentOthers = computed(() => recents.value.filter((r) => r !== agentId.value.trim()));
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
} = useAuth();

// "account" = sign in + pick a device; "manual" = type a server/agent (self-host).
const mode = ref<"account" | "manual">("account");
const loginEmail = ref("");
const loginPassword = ref("");
const isSignup = ref(false);

async function submitAuth() {
  const fn = isSignup.value ? authSignup : authLogin;
  await fn(backend.value.trim(), loginEmail.value.trim(), loginPassword.value);
  if (loggedIn.value) loginPassword.value = "";
}
function connectDevice(d: { id: string; name: string }) {
  rememberAgent(d.id);
  connect(backend.value.trim(), d.id, sessionToken.value || undefined);
}

// Add-device flow (returns the agent token to paste into the device).
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
function copyText(t: string) {
  navigator.clipboard?.writeText(t).catch(() => {});
}

onMounted(() => {
  if (loggedIn.value) refreshDevices(backend.value.trim());
});

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

// The <video> is rendered only once a stream arrives, so attach after render.
watch(videoStream, async (s) => {
  await nextTick();
  if (videoEl.value) videoEl.value.srcObject = s;
});

function toggle() {
  if (active.value) {
    disconnect();
    return;
  }
  const id = agentId.value.trim();
  rememberAgent(id);
  persist();
  connect(backend.value.trim(), id, ticket.value.trim());
}

const dotClass = computed(() => {
  const s = status.value;
  if (s === "streaming" || s === "connected") return "bg-emerald-400";
  if (s.includes("error") || s.includes("offline") || s === "disconnected") return "bg-rose-400";
  if (s === "idle") return "bg-slate-500";
  return "bg-amber-400 animate-pulse";
});

/** Which screen to show: the connection panel, the connecting state, or the live video. */
const phase = computed<"idle" | "connecting" | "live">(() => {
  if (videoStream.value) return "live";
  if (active.value) return "connecting";
  return "idle";
});

/** Friendly one-liner for the connecting state, derived from the raw status. */
const connectingLabel = computed(() => {
  const s = status.value;
  if (s.startsWith("requesting")) return "Waiting for the remote PC to accept…";
  if (s.startsWith("accepted")) return "Accepted — starting the video…";
  if (s === "connecting" || s === "checking" || s === "new") return "Establishing a secure connection…";
  return s.charAt(0).toUpperCase() + s.slice(1);
});

// Chat panel.
const codeInput = ref("");
function onSubmitCode() {
  submitCode(codeInput.value);
  codeInput.value = "";
}

const chatOpen = ref(false);
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
// Auto-scroll to the newest message; clear unread while the panel is open.
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

// Fullscreen the remote-screen stage (Esc exits).
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

// Autofocus the Agent field whenever the connection panel is showing.
const agentInput = ref<HTMLInputElement | null>(null);
function focusAgent() {
  nextTick(() => agentInput.value?.focus());
}
onMounted(() => {
  if (phase.value === "idle") focusAgent();
});
watch(phase, (p) => {
  if (p === "idle") focusAgent();
});

/** Mouse position normalized to the video content (0..1), or null if outside. */
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
  if (now - lastMove < 16) return; // ~60/s
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
  <div class="flex h-full flex-col bg-ink text-slate-200">
    <!-- ── Top bar ─────────────────────────────────────────────────────── -->
    <header
      class="flex h-14 shrink-0 items-center gap-3 border-b border-edge/80 bg-panel/80 px-4 backdrop-blur"
    >
      <div class="flex items-center gap-2.5">
        <span
          class="grid h-7 w-7 place-items-center rounded-lg bg-gradient-to-br from-accent to-accent2 shadow-[0_2px_10px_-2px] shadow-accent/50"
        >
          <Icon name="monitor" class="h-4 w-4 text-white" />
        </span>
        <div class="leading-none">
          <div class="text-[15px] font-semibold tracking-tight text-slate-100">Arna</div>
          <div class="mt-0.5 text-[10px] font-medium uppercase tracking-[0.15em] text-slate-500">Console</div>
        </div>
      </div>

      <!-- Live session info -->
      <template v-if="phase === 'live'">
        <div class="mx-1 h-6 w-px bg-edge" />
        <div class="flex items-center gap-2">
          <span class="h-2 w-2 rounded-full" :class="dotClass" />
          <span class="font-mono text-sm text-slate-300">{{ agentId }}</span>
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

        <span
          v-if="canControl"
          class="flex items-center gap-1.5 rounded-full bg-accent/15 px-2.5 py-1 text-xs font-semibold text-accent2"
        >
          <Icon name="keyboard" class="h-3.5 w-3.5" /> control
        </span>

        <template v-if="phase === 'live'">
          <div
            v-if="monitors.length > 1"
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
            <span
              v-if="unread > 0"
              class="absolute -right-1 -top-1 grid h-4 min-w-4 place-items-center rounded-full bg-accent px-1 text-[10px] font-bold text-white"
            >
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
          <button
            class="flex items-center gap-1.5 rounded-lg bg-rose-500/90 px-3 py-1.5 text-sm font-semibold text-white transition hover:bg-rose-500 focus-visible:ring-2 focus-visible:ring-rose-400"
            @click="toggle"
          >
            <Icon name="x" class="h-4 w-4" /> End
          </button>
        </template>
      </div>
      <input ref="fileInput" type="file" class="hidden" @change="onFileChosen" />
    </header>

    <!-- ── Stage ───────────────────────────────────────────────────────── -->
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
      <!-- Transfer progress (upload or download) -->
      <div
        v-if="transferProgress > 0 && transferProgress < 1"
        class="pointer-events-none absolute inset-x-0 top-0 z-30 h-0.5 bg-edge"
      >
        <div class="h-full bg-accent transition-[width] duration-150" :style="{ width: transferProgress * 100 + '%' }" />
      </div>

      <!-- Drag-to-send overlay -->
      <div
        v-if="dragOver && canSendFiles"
        class="pointer-events-none absolute inset-4 z-30 grid place-items-center rounded-2xl border-2 border-dashed border-accent/70 bg-ink/80 backdrop-blur-sm"
      >
        <div class="text-center">
          <Icon name="upload" class="mx-auto mb-2 h-8 w-8 text-accent2" />
          <p class="font-medium text-slate-100">Drop to send to the remote PC</p>
        </div>
      </div>

      <!-- LIVE: the remote screen -->
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
        <div
          class="pointer-events-none absolute right-4 top-4 z-20 flex items-center gap-2 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-3 py-1 text-xs font-semibold text-emerald-300 backdrop-blur"
        >
          <span class="h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-400" /> LIVE
        </div>
        <div
          v-if="canControl"
          class="pointer-events-none absolute bottom-4 left-1/2 z-20 -translate-x-1/2 rounded-full border border-edge/80 bg-panel/80 px-3.5 py-1.5 text-xs text-slate-400 backdrop-blur"
        >
          Click to control · keystrokes are forwarded
        </div>

        <!-- Chat panel -->
        <transition name="slide">
          <aside
            v-if="chatOpen"
            class="absolute bottom-0 right-0 top-0 z-30 flex w-80 flex-col border-l border-edge bg-panel/95 backdrop-blur"
          >
            <header class="flex items-center gap-2 border-b border-edge px-4 py-3">
              <Icon name="chat" class="h-4 w-4 text-accent2" />
              <span class="text-sm font-semibold text-slate-100">Chat</span>
              <span class="text-xs text-slate-500">with {{ agentId }}</span>
              <button
                class="ml-auto grid h-6 w-6 place-items-center rounded-md text-slate-500 transition hover:bg-ink hover:text-slate-200"
                title="Close"
                @click="toggleChat"
              >
                <Icon name="x" class="h-4 w-4" />
              </button>
            </header>

            <div ref="chatLog" class="flex-1 space-y-2 overflow-y-auto px-3 py-3">
              <p v-if="!messages.length" class="mt-6 px-3 text-center text-sm text-slate-600">
                No messages yet. Say hello to the person at the remote PC.
              </p>
              <div v-for="(m, i) in messages" :key="i" class="flex" :class="m.mine ? 'justify-end' : 'justify-start'">
                <span
                  class="max-w-[85%] whitespace-pre-wrap break-words rounded-2xl px-3 py-1.5 text-sm"
                  :class="m.mine ? 'rounded-br-sm bg-accent text-white' : 'rounded-bl-sm bg-ink text-slate-200'"
                >
                  {{ m.text }}
                </span>
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
              <button
                type="submit"
                :disabled="!draft.trim()"
                class="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-accent text-white transition hover:bg-accent/90 disabled:opacity-40 focus-visible:ring-2 focus-visible:ring-accent"
              >
                <Icon name="send" class="h-4 w-4" />
              </button>
            </form>
          </aside>
        </transition>
      </template>

      <!-- CONNECTING -->
      <div v-else-if="phase === 'connecting'" class="flex flex-col items-center gap-5 px-6 text-center">
        <!-- Require-code: ask the caller for the code the operator reads out -->
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
              autofocus
              class="w-44 rounded-lg border border-edge bg-ink px-3 py-2.5 text-center font-mono text-2xl tracking-[0.3em] text-slate-100 outline-none focus:border-accent focus:ring-2 focus:ring-accent/30"
            />
            <p v-if="codeError" class="text-xs text-rose-300">{{ codeError }}</p>
            <div class="mt-1 flex gap-2">
              <button type="button" class="rounded-lg border border-edge px-4 py-2 text-sm font-medium text-slate-300 transition hover:bg-panel" @click="toggle">Cancel</button>
              <button type="submit" :disabled="codeInput.trim().length < 6" class="rounded-lg bg-accent px-5 py-2 text-sm font-semibold text-white transition hover:bg-accent/90 disabled:opacity-40">Connect</button>
            </div>
          </form>
        </template>

        <!-- Normal connecting -->
        <template v-else>
          <div class="relative grid h-16 w-16 place-items-center">
            <span class="absolute inset-0 animate-spin rounded-full border-2 border-edge border-t-accent" />
            <Icon name="monitor" class="h-6 w-6 text-slate-400" />
          </div>
          <div>
            <p class="text-base font-medium text-slate-100">{{ connectingLabel }}</p>
            <p class="mt-1 font-mono text-sm text-slate-500">{{ agentId }}</p>
          </div>
          <p v-if="sessionCode" class="rounded-lg border border-edge bg-panel px-3 py-2 text-sm text-slate-400">
            If asked for a code, share <span class="font-mono font-semibold text-accent2">{{ sessionCode }}</span>
          </p>
          <button
            class="rounded-lg border border-edge px-4 py-2 text-sm font-medium text-slate-300 transition hover:bg-panel focus-visible:ring-2 focus-visible:ring-accent"
            @click="toggle"
          >
            Cancel
          </button>
        </template>
      </div>

      <!-- IDLE -->
      <div v-else class="w-full max-w-md px-6">
        <!-- ACCOUNT · signed in → your devices -->
        <template v-if="mode === 'account' && loggedIn">
          <div class="rounded-2xl border border-edge bg-panel/90 p-7 shadow-2xl shadow-black/40">
            <div class="mb-5 flex items-center gap-3">
              <span class="grid h-11 w-11 shrink-0 place-items-center rounded-xl bg-accent/10 text-accent2 ring-1 ring-inset ring-accent/20">
                <Icon name="monitor" class="h-5 w-5" />
              </span>
              <div class="min-w-0 flex-1">
                <h1 class="text-lg font-semibold tracking-tight text-slate-100">Your devices</h1>
                <p class="truncate text-sm text-slate-500">{{ accountEmail }}</p>
              </div>
              <button class="grid h-8 w-8 shrink-0 place-items-center rounded-lg border border-edge text-slate-400 transition hover:bg-ink hover:text-slate-200" title="Sign out" @click="authLogout()">
                <Icon name="logout" class="h-4 w-4" />
              </button>
            </div>

            <div v-if="errorMessage" role="alert" class="mb-4 flex items-start gap-2.5 rounded-lg border border-rose-500/30 bg-rose-500/10 px-3 py-2.5 text-sm text-rose-200">
              <Icon :name="errorKind === 'offline' ? 'offline' : 'alert'" class="mt-0.5 h-4 w-4 shrink-0 text-rose-400" />
              <span>{{ errorMessage }}</span>
            </div>

            <div v-if="myDevices.length" class="space-y-2">
              <button
                v-for="d in myDevices"
                :key="d.id"
                class="group flex w-full items-center gap-3 rounded-lg border border-edge bg-ink px-3 py-2.5 text-left transition hover:border-accent"
                @click="connectDevice(d)"
              >
                <span class="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-panel text-slate-400 group-hover:text-accent2">
                  <Icon name="monitor" class="h-4 w-4" />
                </span>
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-sm font-medium text-slate-100">{{ d.name }}</span>
                  <span class="block truncate font-mono text-xs text-slate-500">{{ d.id }}</span>
                </span>
                <Icon name="arrowRightShort" class="h-4 w-4 shrink-0 text-slate-600 group-hover:text-accent2" />
              </button>
            </div>
            <p v-else class="rounded-lg border border-dashed border-edge px-3 py-6 text-center text-sm text-slate-500">
              No devices yet — add one to connect to it.
            </p>

            <button class="mt-4 flex w-full items-center justify-center gap-2 rounded-lg border border-edge px-4 py-2.5 text-sm font-medium text-slate-200 transition hover:bg-ink" @click="showAddDevice = true">
              <Icon name="plus" class="h-4 w-4" /> Add a device
            </button>
          </div>
          <p class="mt-4 text-center text-xs">
            <button class="text-slate-500 underline-offset-2 hover:text-slate-300 hover:underline" @click="mode = 'manual'">Connect manually instead</button>
          </p>
        </template>

        <!-- ACCOUNT · not signed in → login / signup -->
        <template v-else-if="mode === 'account'">
          <div class="rounded-2xl border border-edge bg-panel/90 p-7 shadow-2xl shadow-black/40">
            <div class="mb-5 flex items-center gap-3">
              <span class="grid h-11 w-11 place-items-center rounded-xl bg-gradient-to-br from-accent to-accent2 shadow-lg shadow-accent/25">
                <Icon name="monitor" class="h-5 w-5 text-white" />
              </span>
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
          </div>
          <p class="mt-4 text-center text-xs">
            <button class="text-slate-500 underline-offset-2 hover:text-slate-300 hover:underline" @click="mode = 'manual'">Connect manually instead</button>
          </p>
        </template>

        <!-- MANUAL · type a server + agent id (self-host / advanced) -->
        <template v-else>
          <div class="rounded-2xl border border-edge bg-panel/90 p-7 shadow-2xl shadow-black/40">
            <div class="mb-5 flex items-center gap-3">
              <span class="grid h-11 w-11 place-items-center rounded-xl bg-accent/10 text-accent2 ring-1 ring-inset ring-accent/20">
                <Icon name="monitor" class="h-5 w-5" />
              </span>
              <div>
                <h1 class="text-lg font-semibold tracking-tight text-slate-100">Connect manually</h1>
                <p class="text-sm text-slate-500">Enter the agent ID to start a remote session.</p>
              </div>
            </div>

            <div v-if="errorMessage" role="alert" class="mb-4 flex items-start gap-2.5 rounded-lg border border-rose-500/30 bg-rose-500/10 px-3 py-2.5 text-sm text-rose-200">
              <Icon :name="errorKind === 'offline' ? 'offline' : 'alert'" class="mt-0.5 h-4 w-4 shrink-0 text-rose-400" />
              <span>{{ errorMessage }}</span>
            </div>

            <form class="space-y-3.5" @submit.prevent="toggle">
              <div>
                <label class="mb-1 block text-xs font-medium text-slate-400">Agent ID</label>
                <input ref="agentInput" v-model="agentId" placeholder="agent-1" autocomplete="off" spellcheck="false" class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 text-slate-100 outline-none transition placeholder:text-slate-600 focus:border-accent focus:ring-2 focus:ring-accent/30" />
                <div v-if="recentOthers.length" class="mt-2 flex flex-wrap items-center gap-1.5">
                  <span class="text-xs text-slate-600">Recent</span>
                  <button v-for="r in recentOthers" :key="r" type="button" class="rounded-full border border-edge bg-ink px-2.5 py-1 font-mono text-xs text-slate-400 transition hover:border-slate-600 hover:text-slate-200" @click="agentId = r">{{ r }}</button>
                </div>
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
                <Icon name="arrowRight" class="h-4 w-4 opacity-70" />
              </button>
            </form>
          </div>
          <p class="mt-4 text-center text-xs">
            <button class="text-slate-500 underline-offset-2 hover:text-slate-300 hover:underline" @click="mode = 'account'">← Back to sign in</button>
          </p>
        </template>
      </div>
    </main>

    <!-- Add-device modal -->
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
          <p class="mt-1 text-sm text-slate-400">Device added. On <span class="font-mono text-slate-200">{{ newDeviceId }}</span>, set these and start the Arna agent:</p>
          <div class="mt-4 space-y-2">
            <div class="rounded-lg border border-edge bg-ink px-3 py-2 font-mono text-xs text-slate-300">
              ARNA_AGENT_ID=<span class="text-accent2">{{ newDeviceId }}</span>
            </div>
            <div class="flex items-center gap-2 rounded-lg border border-edge bg-ink px-3 py-2">
              <span class="min-w-0 flex-1 truncate font-mono text-xs text-slate-300">ARNA_AGENT_TOKEN={{ newDeviceToken }}</span>
              <button class="grid h-7 w-7 shrink-0 place-items-center rounded-md border border-edge text-slate-400 hover:text-slate-100" title="Copy token" @click="copyText(newDeviceToken)">
                <Icon name="copy" class="h-3.5 w-3.5" />
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
