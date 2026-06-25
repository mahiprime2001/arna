<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useRemote } from "./composables/useRemote";

const backend = ref("ws://127.0.0.1:8081/ws");
const agentId = ref("agent-1");
const ticket = ref("");
const {
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
  if (active.value) disconnect();
  else connect(backend.value.trim(), agentId.value.trim(), ticket.value.trim());
}

const dotClass = computed(() => {
  const s = status.value;
  if (s === "streaming" || s === "connected") return "bg-emerald-400";
  if (s.includes("error") || s.includes("offline") || s === "disconnected") return "bg-rose-400";
  if (s === "idle") return "bg-slate-500";
  return "bg-amber-400 animate-pulse";
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
  <div class="flex h-full flex-col">
    <!-- Top bar -->
    <header class="flex flex-wrap items-center gap-3 border-b border-edge bg-panel px-4 py-3">
      <div class="flex items-center gap-2">
        <span class="inline-block h-6 w-6 rounded-md bg-gradient-to-br from-accent to-accent2" />
        <span class="text-lg font-semibold tracking-tight">Arna Console</span>
      </div>

      <div class="mx-2 hidden h-6 w-px bg-edge sm:block" />

      <label class="flex items-center gap-2 text-sm text-slate-400">
        Backend
        <input
          v-model="backend"
          :disabled="active"
          class="w-56 rounded-md border border-edge bg-ink px-2 py-1.5 text-slate-100 outline-none focus:border-accent disabled:opacity-50"
        />
      </label>

      <label class="flex items-center gap-2 text-sm text-slate-400">
        Agent
        <input
          v-model="agentId"
          :disabled="active"
          class="w-32 rounded-md border border-edge bg-ink px-2 py-1.5 text-slate-100 outline-none focus:border-accent disabled:opacity-50"
        />
      </label>

      <label class="flex items-center gap-2 text-sm text-slate-400">
        Ticket
        <input
          v-model="ticket"
          :disabled="active"
          placeholder="SSO (optional)"
          class="w-40 rounded-md border border-edge bg-ink px-2 py-1.5 text-slate-100 outline-none focus:border-accent disabled:opacity-50"
        />
      </label>

      <button
        class="rounded-md px-4 py-1.5 text-sm font-semibold transition"
        :class="active ? 'bg-rose-500/90 hover:bg-rose-500 text-white' : 'bg-accent hover:opacity-90 text-white'"
        @click="toggle"
      >
        {{ active ? "Disconnect" : "Connect" }}
      </button>

      <button
        v-if="canSendFiles"
        class="rounded-md border border-edge px-3 py-1.5 text-sm font-medium text-slate-200 transition hover:bg-ink"
        @click="pickFile"
      >
        Send file
      </button>
      <input ref="fileInput" type="file" class="hidden" @change="onFileChosen" />

      <div class="ml-auto flex items-center gap-2 text-sm">
        <span v-if="uploadStatus" class="text-xs text-slate-400">{{ uploadStatus }}</span>
        <span v-if="sessionCode" class="rounded-full bg-slate-700/60 px-2.5 py-0.5 font-mono text-xs text-slate-200">
          code {{ sessionCode }}
        </span>
        <span v-if="canControl" class="rounded-full bg-accent/15 px-2.5 py-0.5 text-xs font-semibold text-accent2">
          control
        </span>
        <span class="h-2.5 w-2.5 rounded-full" :class="dotClass" />
        <span class="font-mono text-slate-400">{{ status }}</span>
      </div>
    </header>

    <!-- Screen area (focusable so it can capture keystrokes) -->
    <main
      ref="screenEl"
      tabindex="0"
      class="relative grid flex-1 place-items-center overflow-hidden bg-black/40 outline-none"
      :class="{ 'cursor-none': canControl && videoStream }"
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
        class="pointer-events-none absolute left-0 right-0 top-0 z-20 h-1 bg-edge"
      >
        <div class="h-full bg-accent transition-[width]" :style="{ width: uploadProgress * 100 + '%' }" />
      </div>

      <!-- Drag-to-send overlay -->
      <div
        v-if="dragOver && canSendFiles"
        class="pointer-events-none absolute inset-3 z-20 grid place-items-center rounded-xl border-2 border-dashed border-accent/70 bg-ink/70 text-slate-200"
      >
        <div class="text-center">
          <div class="mb-1 text-3xl">📁</div>
          <p class="font-medium">Drop to send to the remote PC</p>
        </div>
      </div>
      <video
        v-if="videoStream"
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
      <div v-else class="px-6 text-center text-slate-500">
        <div class="mb-3 text-5xl">🖥️</div>
        <p v-if="!active" class="font-medium">Not connected</p>
        <p v-else class="font-medium">Connecting to <span class="font-mono">{{ agentId }}</span>…</p>
        <p class="mt-1 text-sm">
          Start the backend and an agent, then click <span class="text-slate-300">Connect</span>.
        </p>
      </div>

      <div
        v-if="connected"
        class="pointer-events-none absolute right-3 top-3 flex items-center gap-2 rounded-full bg-emerald-500/15 px-3 py-1 text-xs font-semibold text-emerald-300"
      >
        ● live
        <span v-if="canControl" class="text-slate-300">· click screen to control</span>
      </div>
    </main>
  </div>
</template>
