// Device ownership end-to-end: a user registers a device and gets its agent
// token; only that user's session can connect to it. Run against a backend with
// ARNA_SSO_SECRET set:  node scripts/smoke-devices.mjs

const HTTP = "http://127.0.0.1:8081";
const URL = "ws://127.0.0.1:8081/ws";

const ok = (m) => console.log(`  ✓ ${m}`);
const expect = (c, m) => {
  if (!c) throw new Error("FAILED: " + m);
};
async function post(path, body, token) {
  const headers = { "content-type": "application/json" };
  if (token) headers.authorization = "Bearer " + token;
  const r = await fetch(HTTP + path, { method: "POST", headers, body: JSON.stringify(body) });
  return { status: r.status, json: await r.json().catch(() => null) };
}
async function get(path, token) {
  const r = await fetch(HTTP + path, { headers: { authorization: "Bearer " + token } });
  return { status: r.status, json: await r.json().catch(() => null) };
}
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
function next(ws, ms = 2500) {
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
const sockets = [];

async function run() {
  const ea = `a${Date.now()}@x.com`;
  const eb = `b${Date.now()}@x.com`;
  const pw = "password123";
  const devId = `dev-${Date.now()}`;

  // User A signs up and registers a device.
  const a = await post("/auth/signup", { email: ea, password: pw });
  expect(a.status === 200, "user A signup");
  const tokenA = a.json.token;

  const reg = await post("/devices", { id: devId, name: "A's PC" }, tokenA);
  expect(reg.status === 200 && reg.json.token, `register device (got ${reg.status})`);
  const agentToken = reg.json.token;
  ok("user registers a device and gets its agent token");

  const list = await get("/devices", tokenA);
  expect(list.status === 200 && list.json.some((d) => d.id === devId), "device appears in the list");
  ok("the device shows in the owner's device list");

  // Register without auth -> 401.
  const noauth = await post("/devices", { id: "x", name: "x" });
  expect(noauth.status === 401, `register without auth should be 401 (got ${noauth.status})`);
  ok("registering a device without login is rejected (401)");

  // The agent comes online with its token.
  const agent = await openWs();
  sockets.push(agent);
  send(agent, { type: "register", role: "agent", id: devId, token: agentToken });
  let m = await next(agent);
  expect(m.type === "registered", "agent registers with its device token");
  ok("the device (agent) comes online with its token");

  // Owner connects -> the agent is rung.
  const conA = await openWs();
  sockets.push(conA);
  send(conA, { type: "register", role: "console", id: "viewer-a" });
  await next(conA);
  send(conA, { type: "connect_request", to: devId, ticket: tokenA });
  m = await next(agent);
  expect(m.type === "incoming_request" && m.name === ea, `owner should reach the device (name ${m.name})`);
  ok("the owner can connect (agent sees their email)");

  // A different user can't reach it.
  const b = await post("/auth/signup", { email: eb, password: pw });
  const tokenB = b.json.token;
  const conB = await openWs();
  sockets.push(conB);
  send(conB, { type: "register", role: "console", id: "viewer-b" });
  await next(conB);
  send(conB, { type: "connect_request", to: devId, ticket: tokenB });
  m = await next(conB);
  expect(m.type === "request_denied" && /access/.test(m.reason), `non-owner should be denied (got ${JSON.stringify(m)})`);
  ok("a different user is denied access to the device");

  // Unregistered device id.
  send(conA, { type: "connect_request", to: "ghost-device", ticket: tokenA });
  m = await next(conA);
  expect(m.type === "request_denied" && /not registered/.test(m.reason), "unregistered device should be denied");
  ok("an unregistered device id is denied");

  console.log("\nDEVICE OWNERSHIP OK");
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
