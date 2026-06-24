// Drive the real console app in real Chrome (which has H.264), connect to a
// running agent, and confirm the <video> element actually receives and decodes
// frames. Backend + agent must already be running, console dev server on :4310.
import { chromium } from "playwright";

const CONSOLE_URL = "http://localhost:4310";

async function main() {
  const browser = await chromium.launch({ channel: "chrome", headless: true, args: ["--disable-features=WebRtcHideLocalIpsWithMdns"] });
  const page = await browser.newPage();
  page.on("console", (m) => console.log("  [page]", m.text()));

  try {
    await page.goto(CONSOLE_URL, { waitUntil: "domcontentloaded" });

    // Agent field defaults to agent-1; just click Connect.
    await page.getByRole("button", { name: /connect/i }).click();
    console.log("clicked Connect; waiting for the video track…");

    // Wait until a <video> exists and reports real dimensions (track negotiated
    // + first frame decoded).
    await page.waitForFunction(
      () => {
        const v = document.querySelector("video");
        return !!v && v.videoWidth > 0 && v.videoHeight > 0;
      },
      { timeout: 30000 },
    );
    const dims = await page.evaluate(() => {
      const v = document.querySelector("video");
      return { w: v.videoWidth, h: v.videoHeight };
    });
    console.log(`OK  video track received — ${dims.w}x${dims.h}`);

    // Confirm frames are actually advancing (decoding, not a frozen first frame).
    const t1 = await page.evaluate(() => document.querySelector("video").currentTime);
    await page.waitForTimeout(1500);
    const t2 = await page.evaluate(() => document.querySelector("video").currentTime);
    if (!(t2 > t1)) throw new Error(`video not advancing (currentTime stuck at ${t1})`);
    console.log(`OK  frames decoding — currentTime ${t1.toFixed(2)}s -> ${t2.toFixed(2)}s`);

    console.log("\nVIDEO OK");
    await browser.close();
    process.exit(0);
  } catch (e) {
    console.error("FAILED:", e.message);
    await browser.close();
    process.exit(1);
  }
}

main();
