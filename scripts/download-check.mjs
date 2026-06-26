// Verify file download: console requests -> headless agent serves
// ARNA_DOWNLOAD_FILE -> console saves it. Checks the saved bytes match.
// Backend + console dev server must be running; this script spawns the agent.
import { chromium } from "playwright";
import { spawn } from "child_process";
import { createHash, randomBytes } from "crypto";
import { writeFileSync, readFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";

const AGENT = "D:/Siri-apps/arna-remote/target/release/arna-agent.exe";
const srcPath = join(tmpdir(), "arna-download-src.bin");
const data = randomBytes(640 * 1024);
writeFileSync(srcPath, data);
const srcHash = createHash("sha256").update(data).digest("hex");

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function main() {
  const agent = spawn(AGENT, ["ws://127.0.0.1:8081/ws", "agent-1"], {
    stdio: ["pipe", "pipe", "pipe"],
    env: { ...process.env, ARNA_DOWNLOAD_FILE: srcPath },
  });
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
    for (let i = 0; i < 40 && !out.includes("registered"); i++) await sleep(150);

    let live = false;
    for (let i = 0; i < 6 && !live; i++) {
      await page.goto("http://localhost:4310", { waitUntil: "domcontentloaded" });
      await page.getByRole("button", { name: /^connect$/i }).click();
      try {
        await page.waitForFunction(() => !!document.querySelector("video")?.videoWidth, { timeout: 12000 });
        live = true;
      } catch {}
    }
    if (!live) throw new Error("could not connect");

    // Click the Download button and capture the browser download.
    const dlBtn = page.locator('button[title="Download a file from the remote PC"]');
    await dlBtn.waitFor({ timeout: 8000 });
    const [download] = await Promise.all([
      page.waitForEvent("download", { timeout: 20000 }),
      dlBtn.click(),
    ]);
    const savedPath = join(tmpdir(), "arna-download-got.bin");
    await download.saveAs(savedPath);
    const name = download.suggestedFilename();

    const got = readFileSync(savedPath);
    const gotHash = createHash("sha256").update(got).digest("hex");
    if (got.length !== data.length) throw new Error(`size ${got.length} != ${data.length}`);
    if (gotHash !== srcHash) throw new Error("hash mismatch");
    console.log(`OK  downloaded "${name}" ${got.length} bytes, sha matches (${gotHash.slice(0, 12)}…)`);
    console.log("\nDOWNLOAD OK");
    await cleanup();
    process.exit(0);
  } catch (e) {
    console.error("FAILED:", e.message);
    await cleanup();
    process.exit(1);
  }
}
main();
