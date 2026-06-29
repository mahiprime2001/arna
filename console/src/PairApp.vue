<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// First-run setup: the operator pastes the device id + token from the console's
// "Add a device" screen. We hand them to Rust, which saves them and brings the
// agent online.
const backend = ref("");
const id = ref("");
const token = ref("");

const busy = ref(false);
const error = ref("");
const done = ref<null | "online" | "restart">(null);

onMounted(async () => {
  try {
    const cfg = await invoke<{ backend: string; id: string; token: string }>("current_pairing");
    backend.value = cfg.backend;
    id.value = cfg.id;
  } catch (e) {
    console.error("current_pairing failed", e);
  }
});

async function pair() {
  if (busy.value) return;
  error.value = "";
  if (!id.value.trim() || !token.value.trim()) {
    error.value = "Paste both the device ID and the token.";
    return;
  }
  busy.value = true;
  try {
    const started = await invoke<boolean>("save_pairing", {
      backend: backend.value,
      id: id.value,
      token: token.value,
    });
    done.value = started ? "online" : "restart";
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
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
      <span class="brand">Arna Agent</span>
    </header>

    <!-- Success state -->
    <template v-if="done">
      <span class="check">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
          <path d="M20 6 9 17l-5-5" />
        </svg>
      </span>
      <h1 class="lead">
        {{ done === "online" ? "This PC is online." : "Saved." }}
      </h1>
      <p class="sub">
        <template v-if="done === 'online'">
          Arna is running in the background. You can close this window — it stays
          in the tray, and you'll get a popup whenever someone asks to connect.
        </template>
        <template v-else>
          Quit Arna Agent from the tray and reopen it to start using the new
          device.
        </template>
      </p>
    </template>

    <!-- Form state -->
    <template v-else>
      <p class="kicker">Set up this device</p>
      <h1 class="lead">Connect this PC to your account.</h1>
      <p class="sub">
        In the Arna console, open <strong>Add a device</strong> and copy the ID
        and token it shows. Paste them here.
      </p>

      <label class="field">
        <span class="label">Device ID</span>
        <input
          v-model="id"
          class="input"
          placeholder="front-desk-pc"
          autocapitalize="off"
          autocorrect="off"
          spellcheck="false"
        />
      </label>

      <label class="field">
        <span class="label">Token</span>
        <textarea
          v-model="token"
          class="input mono"
          rows="3"
          placeholder="eyJ0eXAiOiJKV1Qi…"
          spellcheck="false"
        />
      </label>

      <label class="field">
        <span class="label">Server <span class="muted">(advanced)</span></span>
        <input v-model="backend" class="input mono" spellcheck="false" />
      </label>

      <p v-if="error" class="error">{{ error }}</p>

      <button class="btn accept" :disabled="busy" @click="pair">
        <svg v-if="!busy" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M5 12h14M13 5l7 7-7 7" />
        </svg>
        {{ busy ? "Connecting…" : "Bring this PC online" }}
      </button>
    </template>
  </main>
</template>

<style scoped>
.card {
  position: relative;
  min-height: 100%;
  display: flex;
  flex-direction: column;
  padding: 18px 20px 20px;
  overflow: hidden;
  background:
    radial-gradient(120% 70% at 50% -20%, rgba(109, 94, 252, 0.16), transparent 60%),
    #0e121b;
}
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
  font-weight: 600;
  line-height: 1.3;
  color: #f1f5f9;
}
.sub {
  margin: 7px 0 0;
  font-size: 12.5px;
  line-height: 1.45;
  color: #94a3b8;
}
.sub strong {
  color: #cbd5e1;
  font-weight: 600;
}

.field {
  display: block;
  margin-top: 14px;
}
.label {
  display: block;
  margin-bottom: 5px;
  font-size: 11.5px;
  font-weight: 600;
  color: #cbd5e1;
}
.muted {
  color: #64748b;
  font-weight: 400;
}
.input {
  width: 100%;
  box-sizing: border-box;
  padding: 9px 11px;
  border: 1px solid #232a36;
  border-radius: 9px;
  background: #131720;
  color: #f1f5f9;
  font-size: 13px;
  font-family: inherit;
  resize: none;
  transition: border-color 0.15s, box-shadow 0.15s;
}
.input:focus {
  outline: none;
  border-color: #6d5efc;
  box-shadow: 0 0 0 3px rgba(109, 94, 252, 0.18);
}
.input::placeholder {
  color: #475569;
}
.mono {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 12px;
  word-break: break-all;
}

.error {
  margin: 12px 0 0;
  padding: 8px 11px;
  border: 1px solid rgba(251, 113, 133, 0.4);
  border-radius: 9px;
  background: rgba(251, 113, 133, 0.08);
  font-size: 12px;
  color: #fb7185;
}

.btn {
  margin-top: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 11px 0;
  border-radius: 10px;
  border: 1px solid transparent;
  font-size: 14px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: background 0.15s, transform 0.05s, opacity 0.15s;
}
.btn svg {
  width: 16px;
  height: 16px;
}
.btn:active:not(:disabled) {
  transform: translateY(1px);
}
.btn:disabled {
  opacity: 0.55;
  cursor: default;
}
.accept {
  background: #6d5efc;
  color: white;
  box-shadow: 0 6px 18px -6px rgba(109, 94, 252, 0.8);
}
.accept:hover:not(:disabled) {
  background: #5b4ff0;
}

.check {
  display: grid;
  place-items: center;
  width: 46px;
  height: 46px;
  margin: 20px 0 4px;
  border-radius: 50%;
  background: rgba(34, 211, 238, 0.12);
  border: 1px solid rgba(34, 211, 238, 0.4);
  color: #22d3ee;
}
.check svg {
  width: 24px;
  height: 24px;
}

@media (prefers-reduced-motion: reduce) {
  * {
    transition: none !important;
  }
}
</style>
