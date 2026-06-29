<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// The Rust side opens this window with the request details in the query string:
//   index.html?from=<console-id>&name=<admin>&code=<6-digit>
const params = new URLSearchParams(location.search);
const from = params.get("from") ?? "";
const name = params.get("name") || "An administrator";
const code = params.get("code") ?? "";

const busy = ref(false);
const remaining = ref(60);
let timer: number | undefined;

// Tint the countdown as it runs low.
const timerClass = computed(() => (remaining.value <= 5 ? "danger" : remaining.value <= 15 ? "warn" : ""));

async function respond(accept: boolean) {
  if (busy.value) return;
  busy.value = true;
  try {
    await invoke("respond_consent", { id: from, accept, code });
  } catch (e) {
    console.error("respond_consent failed", e);
    busy.value = false;
  }
}

onMounted(() => {
  // Display countdown mirroring the backend's consent timeout (auto-declines).
  timer = window.setInterval(() => {
    remaining.value -= 1;
    if (remaining.value <= 0) respond(false);
  }, 1000);
});
onUnmounted(() => clearInterval(timer));
</script>

<template>
  <main class="card">
    <span class="glow" aria-hidden="true" />

    <header class="head">
      <span class="logo">
        <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">
          <rect width="20" height="14" x="2" y="3" rx="2" /><path d="M8 21h8" /><path d="M12 17v4" />
        </svg>
      </span>
      <span class="brand">Arna</span>
      <span class="timer" :class="timerClass">{{ remaining }}s</span>
    </header>

    <p class="kicker">Connection request</p>
    <h1 class="lead"><strong>{{ name }}</strong> wants to control this PC.</h1>
    <p class="sub">They'll see your screen and can use the mouse and keyboard until you end the session.</p>

    <div v-if="code" class="code">
      <span class="code-label">Session code</span>
      <span class="code-value">{{ code }}</span>
    </div>

    <div class="actions">
      <button class="btn decline" :disabled="busy" @click="respond(false)">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M18 6 6 18M6 6l12 12" />
        </svg>
        Decline
      </button>
      <button class="btn accept" :disabled="busy" @click="respond(true)">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M20 6 9 17l-5-5" />
        </svg>
        Accept
      </button>
    </div>
  </main>
</template>

<style scoped>
.card {
  position: relative;
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 18px 20px 16px;
  overflow: hidden;
  background:
    radial-gradient(120% 80% at 50% -20%, rgba(109, 94, 252, 0.16), transparent 60%),
    #0e121b;
}
/* Soft accent glow bleeding from the top edge. */
.glow {
  position: absolute;
  inset: -40% 20% auto 20%;
  height: 120px;
  background: radial-gradient(closest-side, rgba(34, 211, 238, 0.18), transparent);
  filter: blur(8px);
  pointer-events: none;
}

.head {
  display: flex;
  align-items: center;
  gap: 9px;
}
.logo {
  display: grid;
  place-items: center;
  width: 26px;
  height: 26px;
  border-radius: 8px;
  background: linear-gradient(135deg, #6d5efc, #22d3ee);
  box-shadow: 0 3px 12px -3px rgba(109, 94, 252, 0.7);
}
.logo svg {
  width: 15px;
  height: 15px;
}
.brand {
  font-weight: 600;
  font-size: 15px;
  letter-spacing: -0.01em;
  color: #f1f5f9;
}
.timer {
  margin-left: auto;
  padding: 3px 9px;
  border-radius: 999px;
  border: 1px solid #232a36;
  background: rgba(11, 14, 20, 0.6);
  font-variant-numeric: tabular-nums;
  font-size: 12px;
  color: #8b97a8;
  transition: color 0.2s, border-color 0.2s;
}
.timer.warn {
  color: #fbbf24;
  border-color: rgba(251, 191, 36, 0.4);
}
.timer.danger {
  color: #fb7185;
  border-color: rgba(251, 113, 133, 0.5);
}

.kicker {
  margin: 16px 0 0;
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.16em;
  text-transform: uppercase;
  color: #22d3ee;
}
.lead {
  margin: 5px 0 0;
  font-size: 17px;
  font-weight: 500;
  line-height: 1.3;
  color: #f1f5f9;
}
.lead strong {
  font-weight: 700;
}
.sub {
  margin: 7px 0 0;
  font-size: 12.5px;
  line-height: 1.45;
  color: #94a3b8;
}

.code {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 14px;
  padding: 9px 14px;
  border: 1px solid #232a36;
  border-radius: 10px;
  background: #131720;
}
.code-label {
  font-size: 12px;
  color: #94a3b8;
}
.code-value {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 20px;
  font-weight: 600;
  letter-spacing: 0.22em;
  color: #22d3ee;
}

.actions {
  margin-top: auto;
  padding-top: 16px;
  display: flex;
  gap: 10px;
}
.btn {
  flex: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 10px 0;
  border-radius: 10px;
  border: 1px solid transparent;
  font-size: 14px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: opacity 0.15s, background 0.15s, border-color 0.15s, transform 0.05s;
}
.btn svg {
  width: 16px;
  height: 16px;
}
.btn:active:not(:disabled) {
  transform: translateY(1px);
}
.btn:disabled {
  opacity: 0.5;
  cursor: default;
}
.decline {
  background: transparent;
  border-color: #232a36;
  color: #cbd5e1;
}
.decline:hover:not(:disabled) {
  background: #161b26;
  border-color: #2c3447;
}
.accept {
  background: #6d5efc;
  color: white;
  box-shadow: 0 6px 18px -6px rgba(109, 94, 252, 0.8);
}
.accept:hover:not(:disabled) {
  background: #5b4ff0;
}

@media (prefers-reduced-motion: reduce) {
  * {
    transition: none !important;
  }
}
</style>
