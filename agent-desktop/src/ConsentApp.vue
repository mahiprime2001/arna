<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
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
    <header class="head">
      <span class="logo" />
      <span class="brand">Arna</span>
      <span class="timer">{{ remaining }}s</span>
    </header>

    <p class="lead"><strong>{{ name }}</strong> wants to connect to this PC.</p>
    <p class="sub">They will see your screen and can control it until you end the session.</p>

    <div v-if="code" class="code">
      <span class="code-label">Session code</span>
      <span class="code-value">{{ code }}</span>
    </div>

    <div class="actions">
      <button class="btn decline" :disabled="busy" @click="respond(false)">Decline</button>
      <button class="btn accept" :disabled="busy" @click="respond(true)">Accept</button>
    </div>
  </main>
</template>

<style scoped>
.card {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px 18px;
}
.head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.logo {
  width: 18px;
  height: 18px;
  border-radius: 5px;
  background: linear-gradient(135deg, #6d5efc, #22d3ee);
}
.brand {
  font-weight: 600;
  letter-spacing: -0.01em;
}
.timer {
  margin-left: auto;
  font-variant-numeric: tabular-nums;
  font-size: 12px;
  color: #64748b;
}
.lead {
  margin: 4px 0 0;
  font-size: 15px;
}
.sub {
  margin: 0;
  font-size: 12.5px;
  line-height: 1.4;
  color: #94a3b8;
}
.code {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 2px;
  padding: 8px 12px;
  border: 1px solid #232a36;
  border-radius: 8px;
  background: #131720;
}
.code-label {
  font-size: 12px;
  color: #94a3b8;
}
.code-value {
  font-family: ui-monospace, Menlo, monospace;
  font-size: 18px;
  letter-spacing: 0.18em;
  color: #22d3ee;
}
.actions {
  margin-top: auto;
  display: flex;
  gap: 10px;
}
.btn {
  flex: 1;
  padding: 9px 0;
  border-radius: 8px;
  border: 1px solid transparent;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: opacity 0.15s, background 0.15s;
}
.btn:disabled {
  opacity: 0.5;
  cursor: default;
}
.decline {
  background: transparent;
  border-color: #232a36;
  color: #e2e8f0;
}
.decline:hover:not(:disabled) {
  background: #1a1f2b;
}
.accept {
  background: #6d5efc;
  color: white;
}
.accept:hover:not(:disabled) {
  opacity: 0.9;
}
</style>
