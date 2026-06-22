<script setup lang="ts">
const modules = [
  {
    name: "Remote",
    desc: "Full control of any store PC — screen, mouse, keyboard, multi-monitor.",
    tag: "core",
  },
  {
    name: "Fleet",
    desc: "Live health of every store — disk, queues, status — and one-click remote fixes.",
    tag: "monitor",
  },
  {
    name: "Chat",
    desc: "Live and persistent messaging, with broadcast to every store at once.",
    tag: "messaging",
  },
  {
    name: "Meet",
    desc: "Audio and video calls with screen share — built on the same engine.",
    tag: "calls",
  },
  {
    name: "Files",
    desc: "Drag-and-drop transfer, peer-to-peer for big files, both directions.",
    tag: "transfer",
  },
];

const year = new Date().getFullYear();
</script>

<template>
  <main class="min-h-screen">
    <!-- Nav -->
    <header class="mx-auto flex max-w-6xl items-center justify-between px-6 py-6">
      <div class="flex items-center gap-2">
        <span class="inline-block h-6 w-6 rounded-md bg-gradient-to-br from-accent to-accent2" />
        <span class="text-lg font-semibold tracking-tight">Arna</span>
      </div>
      <nav class="flex items-center gap-6 text-sm text-zinc-400">
        <a href="#modules" class="hover:text-white">Platform</a>
        <a
          href="#download"
          class="rounded-lg bg-white/10 px-3 py-1.5 text-white hover:bg-white/20"
          >Download</a
        >
      </nav>
    </header>

    <!-- Hero -->
    <section class="backdrop-grid">
      <div class="mx-auto max-w-6xl px-6 py-28 text-center">
        <p
          class="mb-4 inline-block rounded-full border border-white/10 bg-white/5 px-3 py-1 text-xs uppercase tracking-widest text-zinc-400"
        >
          Self-hosted · end-to-end encrypted
        </p>
        <h1 class="mx-auto max-w-3xl text-5xl font-semibold leading-tight tracking-tight md:text-6xl">
          Remote control for your
          <span class="bg-gradient-to-r from-accent to-accent2 bg-clip-text text-transparent">
            entire store network
          </span>
        </h1>
        <p class="mx-auto mt-6 max-w-xl text-lg text-zinc-400">
          One app to support, monitor, message, and meet across every store —
          running on your own infrastructure, owned end to end.
        </p>
        <div class="mt-10 flex items-center justify-center gap-4">
          <a
            href="#download"
            class="rounded-xl bg-gradient-to-r from-accent to-accent2 px-6 py-3 font-medium text-ink shadow-lg shadow-accent/20 transition hover:opacity-90"
            >Download the app</a
          >
          <a
            href="#modules"
            class="rounded-xl border border-white/10 px-6 py-3 font-medium text-zinc-200 hover:bg-white/5"
            >Explore the platform</a
          >
        </div>
      </div>
    </section>

    <!-- Modules -->
    <section id="modules" class="mx-auto max-w-6xl px-6 py-24">
      <h2 class="mb-3 text-center text-3xl font-semibold tracking-tight">One app, many tools</h2>
      <p class="mx-auto mb-14 max-w-xl text-center text-zinc-400">
        Every module runs on the same WebRTC engine — so it's one platform, not five products.
      </p>
      <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <div
          v-for="m in modules"
          :key="m.name"
          class="glow-border rounded-2xl bg-panel p-6 transition hover:border-white/20"
        >
          <div class="mb-3 flex items-center justify-between">
            <h3 class="text-lg font-semibold">{{ m.name }}</h3>
            <span class="rounded-md bg-white/5 px-2 py-0.5 font-mono text-xs text-zinc-500">{{
              m.tag
            }}</span>
          </div>
          <p class="text-sm leading-relaxed text-zinc-400">{{ m.desc }}</p>
        </div>
        <div
          class="glow-border flex items-center justify-center rounded-2xl bg-gradient-to-br from-accent/15 to-accent2/10 p-6 text-center"
        >
          <p class="text-sm text-zinc-300">More tools, same foundation — built phase by phase.</p>
        </div>
      </div>
    </section>

    <!-- Download -->
    <section id="download" class="mx-auto max-w-6xl px-6 pb-28">
      <div class="glow-border rounded-3xl bg-panel p-12 text-center">
        <h2 class="text-3xl font-semibold tracking-tight">Get the Console</h2>
        <p class="mx-auto mt-3 max-w-md text-zinc-400">
          Install the Console to connect, and the Agent on each store PC. Builds are published per
          release.
        </p>
        <div
          class="mt-8 inline-flex rounded-xl border border-white/10 bg-white/5 px-5 py-3 font-mono text-sm text-zinc-400"
        >
          Windows build — coming with the first release
        </div>
      </div>
    </section>

    <!-- Footer -->
    <footer class="border-t border-white/5">
      <div
        class="mx-auto flex max-w-6xl flex-col items-center justify-between gap-3 px-6 py-8 text-sm text-zinc-500 sm:flex-row"
      >
        <span>© {{ year }} Arna · Siri ecosystem</span>
        <span class="font-mono text-xs">self-hosted · private</span>
      </div>
    </footer>
  </main>
</template>
