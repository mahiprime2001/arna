// Send a file from the console to the agent and verify it lands byte-identical
// in ~/ArnaRemote/Incoming. Backend + agent + console dev server must be up.
import { chromium } from "playwright";
import { createHash, randomBytes } from "crypto";
import { writeFileSync, readFileSync, existsSync, readdirSync, rmSync, mkdirSync } from "fs";
import { homedir, tmpdir } from "os";
import { join } from "path";

const CONSOLE_URL = "http://localhost:4310";
const NAME = "arna-testfile.bin";
const tmpFile = join(tmpdir(), NAME);
const incomingDir = join(homedir(), "ArnaRemote", "Incoming");
const savedPath = join(incomingDir, NAME);

// 1) Make a ~700 KB random test file (spans many chunks) and hash it.
const data = randomBytes(700 * 1024);
writeFileSync(tmpFile, data);
const srcHash = createHash("sha256").update(data).digest("hex");

// Clean any prior copy so the agent saves exactly NAME (not "NAME (1)").
mkdirSync(incomingDir, { recursive: true });
for (const f of readdirSync(incomingDir)) {
  if (f.startsWith("arna-testfile")) rmSync(join(incomingDir, f));
}

async function tryConnect(page) {
  await page.goto(CONSOLE_URL, { waitUntil: "domcontentloaded" });
  await page.getByRole("button", { name: /^connect$/i }).click();
  try {
    await page.waitForFunction(() => !!document.querySelector("video")?.videoWidth, { timeout: 12000 });
    return true;
  } catch {
    return false;
  }
}

async function main() {
  const browser = await chromium.launch({
    channel: "chrome",
    headless: true,
    args: ["--disable-features=WebRtcHideLocalIpsWithMdns"],
  });
  const page = await browser.newPage();
  page.on("console", (m) => m.text().includes("error") && console.log("  [page]", m.text()));

  let ok = false;
  for (let i = 1; i <= 5 && !ok; i++) ok = await tryConnect(page);
  if (!ok) {
    console.error("FAILED: could not connect");
    await browser.close();
    process.exit(1);
  }

  // Wait until the files channel is open (Send file button appears), then send.
  await page.getByRole("button", { name: /send file/i }).waitFor({ timeout: 10000 });
  await page.setInputFiles('input[type="file"]', tmpFile);
  console.log(`sending ${NAME} (${data.length} bytes, sha ${srcHash.slice(0, 12)}…)`);

  await page.waitForFunction(() => document.body.innerText.includes("saved on remote"), { timeout: 30000 });
  console.log("console reports: saved on remote");
  await browser.close();

  // 2) Verify the saved file matches.
  if (!existsSync(savedPath)) {
    console.error(`FAILED: ${savedPath} does not exist`);
    process.exit(1);
  }
  const got = readFileSync(savedPath);
  const dstHash = createHash("sha256").update(got).digest("hex");
  if (got.length !== data.length) {
    console.error(`FAILED: size ${got.length} != ${data.length}`);
    process.exit(1);
  }
  if (dstHash !== srcHash) {
    console.error(`FAILED: hash mismatch`);
    process.exit(1);
  }
  console.log(`OK  saved ${got.length} bytes, sha matches (${dstHash.slice(0, 12)}…)`);
  console.log("\nFILE TRANSFER OK");
  process.exit(0);
}
main();
