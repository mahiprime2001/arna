<script setup lang="ts">
import { computed, ref } from "vue";
import { useRemote } from "./composables/useRemote";

const backend = ref("ws://127.0.0.1:8081/ws");
const agentId = ref("agent-1");
const { status, active, connected, screenUrl, connect, disconnect } = useRemote();

function toggle() {
  if (active.value) disconnect();
  else connect(backend.value.trim(), agentId.value.trim());
}

const dotClass = computed(() => {
  const s = status.value;
  if (s === "streaming" || s === "connected") return "bg-emerald-400";
  if (s.includes("error") || s.includes("offline") || s === "disconnected") return "bg-rose-400";
  if (s === "idle") return "bg-slate-500";
  return "bg-amber-400 animate-pulse";
});
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

      <button
        class="rounded-md px-4 py-1.5 text-sm font-semibold transition"
        :class="active ? 'bg-rose-500/90 hover:bg-rose-500 text-white' : 'bg-accent hover:opacity-90 text-white'"
        @click="toggle"
      >
        {{ active ? "Disconnect" : "Connect" }}
      </button>

      <div class="ml-auto flex items-center gap-2 text-sm">
        <span class="h-2.5 w-2.5 rounded-full" :class="dotClass" />
        <span class="font-mono text-slate-400">{{ status }}</span>
      </div>
    </header>

    <!-- Screen area -->
    <main class="relative grid flex-1 place-items-center overflow-auto bg-black/40">
      <img
        v-if="screenUrl"
        :src="screenUrl"
        alt="remote screen"
        class="max-h-full max-w-full object-contain"
        draggable="false"
      />
      <div v-else class="px-6 text-center text-slate-500">
        <div class="mb-3 text-5xl">🖥️</div>
        <p v-if="!active" class="font-medium">Not connected</p>
        <p v-else class="font-medium">Connecting to <span class="font-mono">{{ agentId }}</span>…</p>
        <p class="mt-1 text-sm">
          Start the backend and an agent, then click <span class="text-slate-300">Connect</span>.
        </p>
      </div>

      <span
        v-if="connected"
        class="absolute right-3 top-3 rounded-full bg-emerald-500/15 px-3 py-1 text-xs font-semibold text-emerald-300"
      >
        ● live
      </span>
    </main>
  </div>
</template>
