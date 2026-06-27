// Verify require-code consent: the agent (ARNA_CONSENT=code) makes the caller
// type the 6-digit code it shows. Wrong code is refused; right code admits.
// Spawns the agent; an OPEN-mode backend must already be running on :8081.
import { spawn } from "child_process";

const URL = "ws://127.0.0.1:8081/ws";
const AGENT = "D:/Siri-apps/arna-remote/target/release/arna-agent.exe";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function openWs() {
  return new Promise((resolve) => {
    const ws = new WebSocket(URL);
    ws.inbox = [];
    ws.waiters = [];
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      const w = ws.waiters.shift();
      if (w) w(m);
      else ws.inbox.push(m);
    };
    ws.onopen = () => resolve(ws);
  });
}
function next(ws, ms = 3000) {
  if (ws.inbox.length) return Promise.resolve(ws.inbox.shift());
  return new Promise((res, rej) => {
    const t = setTimeout(() => rej(new Error("timeout")), ms);
    ws.waiters.push((m) => {
      clearTimeout(t);
      res(m);
    });
  });
}
const send = (ws, o) => ws.send(JSON.stringify(o));
const ok = (m) => console.log(`  ✓ ${m}`);
const expect = (c, m) => {
  if (!c) throw new Error("FAILED: " + m);
};

async function main() {
  const agent = spawn(AGENT, [URL, "agent-1"], {
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, ARNA_CONSENT: "code" },
  });
  let out = "";
  agent.stdout.on("data", (d) => (out += d.toString()));

  try {
    for (let i = 0; i < 40 && !out.includes("registered"); i++) await sleep(150);

    const con = await openWs();
    send(con, { type: "register", role: "console", id: "viewer-code" });
    await next(con);
    send(con, { type: "connect_request", to: "agent-1" });

    const consent = await next(con, 4000);
    expect(consent.type === "signal" && consent.data.kind === "consent" && consent.data.require_code, "should require a code");
    ok("agent requires a code before admitting");

    // Read the real code from the agent's output.
    for (let i = 0; i < 20 && !/must enter code (\d{6})/.test(out); i++) await sleep(100);
    const real = out.match(/must enter code (\d{6})/)?.[1];
    expect(real, "agent should print the code");

    // Wrong code -> code_bad (not final).
    const wrong = real === "000000" ? "111111" : "000000";
    send(con, { type: "signal", to: "agent-1", data: { kind: "code", code: wrong } });
    let m = await next(con, 3000);
    expect(m.data?.kind === "code_bad" && m.data.final === false, "wrong code should be refused");
    ok("wrong code is refused (retry allowed)");

    // Right code -> code_ok.
    send(con, { type: "signal", to: "agent-1", data: { kind: "code", code: real } });
    m = await next(con, 3000);
    expect(m.data?.kind === "code_ok", "correct code should be accepted");
    ok(`correct code (${real}) admits the caller`);

    console.log("\nREQUIRE-CODE OK");
    agent.kill();
    process.exit(0);
  } catch (e) {
    console.error(e.message, "\n--- agent ---\n", out.split("\n").filter((l) => !l.includes("tungstenite")).slice(-8).join("\n"));
    agent.kill();
    process.exit(1);
  }
}
main();
