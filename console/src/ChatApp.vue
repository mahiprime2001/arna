<script setup lang="ts">
import { nextTick, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type ChatMsg = { id: number; mine: boolean; text: string };

const messages = ref<ChatMsg[]>([]);
const draft = ref("");
const log = ref<HTMLDivElement | null>(null);
const seen = new Set<number>();

function add(m: ChatMsg) {
  if (seen.has(m.id)) return;
  seen.add(m.id);
  messages.value.push(m);
  nextTick(() => {
    if (log.value) log.value.scrollTop = log.value.scrollHeight;
  });
}

async function send() {
  const text = draft.value.trim();
  if (!text) return;
  draft.value = "";
  // The reply echoes back via the chat://msg event, so we don't add it here.
  await invoke("send_chat", { text });
}

onMounted(async () => {
  try {
    const history = await invoke<ChatMsg[]>("chat_history");
    history.forEach(add);
  } catch (e) {
    console.error("chat_history failed", e);
  }
  await listen<ChatMsg>("chat://msg", (e) => add(e.payload));
});
</script>

<template>
  <main class="chat">
    <header class="head">
      <span class="logo">
        <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
        </svg>
      </span>
      <div>
        <div class="title">Chat</div>
        <div class="subtitle">with the other person</div>
      </div>
    </header>

    <div ref="log" class="log">
      <p v-if="!messages.length" class="empty">No messages yet.</p>
      <div v-for="m in messages" :key="m.id" class="row" :class="m.mine ? 'right' : 'left'">
        <span class="bubble" :class="m.mine ? 'mine' : 'theirs'">{{ m.text }}</span>
      </div>
    </div>

    <form class="composer" @submit.prevent="send">
      <input v-model="draft" placeholder="Type a message…" autocomplete="off" @keydown.enter.prevent="send" />
      <button type="submit" :disabled="!draft.trim()" aria-label="Send">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 2 11 13" /><path d="m22 2-7 20-4-9-9-4Z" />
        </svg>
      </button>
    </form>
  </main>
</template>

<style scoped>
.chat {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: #0e121b;
}
.head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-bottom: 1px solid #232a36;
}
.logo {
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border-radius: 8px;
  background: linear-gradient(135deg, #6d5efc, #22d3ee);
  box-shadow: 0 3px 12px -3px rgba(109, 94, 252, 0.7);
}
.logo svg {
  width: 16px;
  height: 16px;
}
.title {
  font-size: 14px;
  font-weight: 600;
  color: #f1f5f9;
  line-height: 1.1;
}
.subtitle {
  font-size: 11.5px;
  color: #8b97a8;
}

.log {
  flex: 1;
  overflow-y: auto;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.empty {
  margin: auto;
  font-size: 13px;
  color: #64748b;
}
.row {
  display: flex;
}
.row.right {
  justify-content: flex-end;
}
.row.left {
  justify-content: flex-start;
}
.bubble {
  max-width: 80%;
  padding: 7px 12px;
  border-radius: 14px;
  font-size: 13.5px;
  line-height: 1.4;
  white-space: pre-wrap;
  word-break: break-word;
}
.bubble.mine {
  background: #6d5efc;
  color: white;
  border-bottom-right-radius: 4px;
}
.bubble.theirs {
  background: #1a2030;
  color: #e2e8f0;
  border-bottom-left-radius: 4px;
}

.composer {
  display: flex;
  gap: 8px;
  padding: 12px;
  border-top: 1px solid #232a36;
}
.composer input {
  flex: 1;
  min-width: 0;
  padding: 9px 12px;
  border-radius: 9px;
  border: 1px solid #232a36;
  background: #0b0e14;
  color: #f1f5f9;
  font-size: 13.5px;
  font-family: inherit;
  outline: none;
  transition: border-color 0.15s;
}
.composer input::placeholder {
  color: #4b5566;
}
.composer input:focus {
  border-color: #6d5efc;
}
.composer button {
  display: grid;
  place-items: center;
  width: 38px;
  height: 38px;
  border: none;
  border-radius: 9px;
  background: #6d5efc;
  color: white;
  cursor: pointer;
  transition: opacity 0.15s, background 0.15s;
}
.composer button svg {
  width: 17px;
  height: 17px;
}
.composer button:disabled {
  opacity: 0.4;
  cursor: default;
}
.composer button:hover:not(:disabled) {
  background: #5b4ff0;
}

@media (prefers-reduced-motion: reduce) {
  * {
    transition: none !important;
  }
}
</style>
