// Security smoke test for the hardening pass: agent authentication + role-aware
// routing. Run against an SSO-enabled backend:
//   ARNA_SSO_SECRET=dev ARNA_DEV_TICKETS=1 ./arna-backend.exe
//   node scripts/smoke-auth.mjs

const URL = "ws://127.0.0.1:8081/ws";
const HTTP = "http://127.0.0.1:8081";

function open() {
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
function next(ws, ms = 2000) {
  if (ws.inbox.length) return Promise.resolve(ws.inbox.shift());
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error("timeout")), ms);
    ws.waiters.push((m) => {
      clearTimeout(t);
      resolve(m);
    });
  });
}
const send = (ws, o) => ws.send(JSON.stringify(o));
async function devToken(params) {
  const r = await fetch(`${HTTP}/dev/ticket?${params}`);
  if (!r.ok) throw new Error(`dev token (${params}) -> ${r.status} (need SSO + ARNA_DEV_TICKETS=1)`);
  return (await r.json()).token;
}
const ok = (m) => console.log(`  ✓ ${m}`);
function expect(cond, msg) {
  if (!cond) throw new Error("FAILED: " + msg);
}

const sockets = [];
async function run() {
  // 1) Agent registration without a token is rejected.
  const a0 = await open();
  sockets.push(a0);
  send(a0, { type: "register", role: "agent", id: "agent-1" });
  let m = await next(a0);
  expect(m.type === "error" && /registration denied/.test(m.message), "agent w/o token should be denied");
  ok("agent registration without a token is rejected");

  // 2) Agent with a valid token registers.
  const agentToken = await devToken("role=agent&id=agent-1");
  const agent = await open();
  sockets.push(agent);
  send(agent, { type: "register", role: "agent", id: "agent-1", token: agentToken });
  m = await next(agent);
  expect(m.type === "registered", "agent with token should register");
  ok("agent with a valid token registers");

  // 3) A console's connect_request routes to the agent.
  const con = await open();
  sockets.push(con);
  send(con, { type: "register", role: "console", id: "viewer-1" });
  await next(con);
  const ticket = await devToken("agent=agent-1&name=Tarun");
  send(con, { type: "connect_request", to: "agent-1", ticket });
  m = await next(agent);
  expect(m.type === "incoming_request" && m.name === "Tarun", "agent should get incoming_request");
  ok("console connect_request reaches the authenticated agent");

  // 4) An impostor console can't take over the agent's id.
  const imp = await open();
  sockets.push(imp);
  send(imp, { type: "register", role: "console", id: "agent-1" });
  m = await next(imp);
  expect(m.type === "error" && /in use by a device/.test(m.message), "impostor should be blocked");
  ok("a console can't hijack a live agent id");

  // 5) connect_request to a non-agent (a console) is refused.
  const con2 = await open();
  sockets.push(con2);
  send(con2, { type: "register", role: "console", id: "viewer-2" });
  await next(con2);
  const t2 = await devToken("agent=viewer-1&name=X");
  send(con2, { type: "connect_request", to: "viewer-1", ticket: t2 });
  m = await next(con2);
  expect(m.type === "request_denied" && m.reason === "not an agent", "should deny connect to a console");
  ok("connect_request to a non-agent is refused");

  // 6) Spamming connect_requests gets rate-limited.
  const spam = await open();
  sockets.push(spam);
  send(spam, { type: "register", role: "console", id: "viewer-spam" });
  await next(spam);
  const st = await devToken("agent=agent-1&name=Spam");
  for (let i = 0; i < 14; i++) send(spam, { type: "connect_request", to: "agent-1", ticket: st });
  await new Promise((r) => setTimeout(r, 500));
  const throttled = spam.inbox.filter((x) => x.type === "request_denied" && /too many/.test(x.reason || ""));
  expect(throttled.length > 0, "connect_request spam should be throttled");
  ok(`connect_request spam is rate-limited (${throttled.length} refused)`);

  console.log("\nAUTH HARDENING OK");
}

function shutdown(code) {
  for (const ws of sockets) {
    ws.onmessage = ws.onerror = ws.onclose = null;
    try {
      ws.close();
    } catch {}
  }
  setTimeout(() => process.exit(code), 150);
}
run().then(
  () => shutdown(0),
  (e) => {
    console.error(e.message);
    shutdown(1);
  },
);
