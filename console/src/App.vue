<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useRemote } from "./composables/useRemote";
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

const backend = ref(saved.backend || "ws://127.0.0.1:8081/ws");
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
  canSendFiles,
  uploadProgress,
  uploadStatus,
  canChat,
  messages,
  unread,
  connect,
  disconnect,
  sendInput,
  sendFile,
  sendChat,
  markChatRead,
} = useRemote();

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
        <span v-if="uploadStatus" class="hidden text-xs text-slate-400 sm:inline">{{ uploadStatus }}</span>

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
      @contextmenu="onContextMenu"
      @dragover.prevent="onDragOver"
      @dragleave="dragOver = false"
      @drop.prevent="onDrop"
    >
      <!-- Upload progress -->
      <div
        v-if="uploadProgress > 0 && uploadProgress < 1"
        class="pointer-events-none absolute inset-x-0 top-0 z-30 h-0.5 bg-edge"
      >
        <div class="h-full bg-accent transition-[width] duration-150" :style="{ width: uploadProgress * 100 + '%' }" />
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
        <div class="relative grid h-16 w-16 place-items-center">
          <span class="absolute inset-0 animate-spin rounded-full border-2 border-edge border-t-accent" />
          <Icon name="monitor" class="h-6 w-6 text-slate-400" />
        </div>
        <div>
          <p class="text-base font-medium text-slate-100">{{ connectingLabel }}</p>
          <p class="mt-1 font-mono text-sm text-slate-500">{{ agentId }}</p>
        </div>
        <p
          v-if="sessionCode"
          class="rounded-lg border border-edge bg-panel px-3 py-2 text-sm text-slate-400"
        >
          If asked for a code, share <span class="font-mono font-semibold text-accent2">{{ sessionCode }}</span>
        </p>
        <button
          class="rounded-lg border border-edge px-4 py-2 text-sm font-medium text-slate-300 transition hover:bg-panel focus-visible:ring-2 focus-visible:ring-accent"
          @click="toggle"
        >
          Cancel
        </button>
      </div>

      <!-- IDLE: connection panel -->
      <div v-else class="w-full max-w-md px-6">
        <div class="rounded-2xl border border-edge bg-panel/90 p-7 shadow-2xl shadow-black/40">
          <div class="mb-5 flex items-center gap-3">
            <span class="grid h-11 w-11 place-items-center rounded-xl bg-accent/10 text-accent2 ring-1 ring-inset ring-accent/20">
              <Icon name="monitor" class="h-5 w-5" />
            </span>
            <div>
              <h1 class="text-lg font-semibold tracking-tight text-slate-100">Connect to a machine</h1>
              <p class="text-sm text-slate-500">Enter the agent ID to start a remote session.</p>
            </div>
          </div>

          <div
            v-if="errorMessage"
            role="alert"
            class="mb-4 flex items-start gap-2.5 rounded-lg border border-rose-500/30 bg-rose-500/10 px-3 py-2.5 text-sm text-rose-200"
          >
            <Icon :name="errorKind === 'offline' ? 'offline' : 'alert'" class="mt-0.5 h-4 w-4 shrink-0 text-rose-400" />
            <span>{{ errorMessage }}</span>
          </div>

          <form class="space-y-3.5" @submit.prevent="toggle">
            <div>
              <label class="mb-1 block text-xs font-medium text-slate-400">Agent ID</label>
              <input
                ref="agentInput"
                v-model="agentId"
                placeholder="agent-1"
                autocomplete="off"
                spellcheck="false"
                class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 text-slate-100 outline-none transition placeholder:text-slate-600 focus:border-accent focus:ring-2 focus:ring-accent/30"
              />
              <div v-if="recentOthers.length" class="mt-2 flex flex-wrap items-center gap-1.5">
                <span class="text-xs text-slate-600">Recent</span>
                <button
                  v-for="r in recentOthers"
                  :key="r"
                  type="button"
                  class="rounded-full border border-edge bg-ink px-2.5 py-1 font-mono text-xs text-slate-400 transition hover:border-slate-600 hover:text-slate-200"
                  @click="agentId = r"
                >
                  {{ r }}
                </button>
              </div>
            </div>
            <div>
              <label class="mb-1 block text-xs font-medium text-slate-400">Signaling server</label>
              <input
                v-model="backend"
                class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 font-mono text-sm text-slate-300 outline-none transition focus:border-accent focus:ring-2 focus:ring-accent/30"
              />
            </div>
            <div>
              <label class="mb-1 block text-xs font-medium text-slate-400">
                SSO ticket <span class="text-slate-600">· optional</span>
              </label>
              <input
                v-model="ticket"
                placeholder="paste a signed ticket if required"
                class="w-full rounded-lg border border-edge bg-ink px-3 py-2.5 text-sm text-slate-300 outline-none transition placeholder:text-slate-600 focus:border-accent focus:ring-2 focus:ring-accent/30"
              />
            </div>
            <button
              type="submit"
              class="mt-1 flex w-full items-center justify-center gap-2 rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-white shadow-lg shadow-accent/25 transition hover:bg-accent/90 focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-panel"
            >
              <Icon name="power" class="h-4 w-4" /> Connect
              <Icon name="arrowRight" class="h-4 w-4 opacity-70" />
            </button>
          </form>
        </div>
        <p class="mt-4 text-center text-xs text-slate-600">
          The remote PC must accept the connection before the screen appears.
        </p>
      </div>
    </main>
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
