// Two-way chat check: console <-> headless agent.
// Spawns the agent (to control its stdin), connects the console in Chrome,
// sends a message each way, and verifies both arrive. Backend + console dev
// server must already be running.
import { chromium } from "playwright";
import { spawn } from "child_process";

const AGENT = "D:\\Siri-apps\\arna-remote\\target\\release\\arna-agent.exe";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function waitFor(fn, ms, label) {
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    if (await fn()) return;
    await sleep(150);
  }
  throw new Error("timeout: " + label);
}

async function main() {
  const agent = spawn(AGENT, ["ws://127.0.0.1:8081/ws", "agent-1"], { stdio: ["pipe", "pipe", "pipe"] });
  let out = "";
  agent.stdout.on("data", (d) => (out += d.toString()));

  const browser = await chromium.launch({
    channel: "chrome",
    headless: true,
    args: ["--disable-features=WebRtcHideLocalIpsWithMdns"],
  });
  const page = await browser.newPage();

  const cleanup = async () => {
    await browser.close().catch(() => {});
    agent.kill();
  };
  try {
    await waitFor(async () => out.includes("registered"), 6000, "agent register");

    // Connect (retry past single-machine ICE flakiness).
    let live = false;
    for (let i = 1; i <= 5 && !live; i++) {
      await page.goto("http://localhost:4310", { waitUntil: "domcontentloaded" });
      await page.getByRole("button", { name: /^connect$/i }).click();
      try {
        await page.waitForFunction(() => !!document.querySelector("video")?.videoWidth, { timeout: 12000 });
        live = true;
      } catch {}
    }
    if (!live) throw new Error("could not connect");

    // Open the chat panel.
    await page.locator('button[title="Chat"]').waitFor({ timeout: 8000 });
    await page.locator('button[title="Chat"]').click();

    // console -> agent
    await page.getByPlaceholder(/Type a message/).fill("hello from admin");
    await page.locator('aside form button[type="submit"]').click();
    const consoleSees = await page
      .waitForFunction(() => document.querySelector("aside")?.innerText.includes("hello from admin"), { timeout: 2500 })
      .then(() => true)
      .catch(() => false);
    console.log("console rendered its own message:", consoleSees);
    await waitFor(async () => out.includes("admin: hello from admin"), 6000, "console->agent");
    console.log("OK  console -> agent (agent printed the message)");

    // agent -> console
    agent.stdin.write("hi from the store\n");
    await page.waitForFunction(() => document.body.innerText.includes("hi from the store"), { timeout: 6000 });
    console.log("OK  agent -> console (message shows in the chat panel)");

    console.log("\nCHAT OK");
    await cleanup();
    process.exit(0);
  } catch (e) {
    console.error("FAILED:", e.message);
    console.error("--- agent output ---\n" + out.split("\n").filter((l) => !l.includes("frames")).slice(-25).join("\n"));
    await cleanup();
    process.exit(1);
  }
}
main();
