// Signaling-level smoke test for the Phase 3 consent + SSO handshake.
// Spins up a fake agent + fake console against a running backend and asserts
// the message flow. Run with the backend already listening on :8081.

const URL = "ws://127.0.0.1:8081/ws";
const HTTP = "http://127.0.0.1:8081";

const sockets = [];
function open(id, role) {
  return new Promise((resolve) => {
    const ws = new WebSocket(URL);
    sockets.push(ws);
    ws.id = id;
    ws.role = role;
    ws.inbox = [];
    ws.waiters = [];
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      const w = ws.waiters.shift();
      if (w) w(m);
      else ws.inbox.push(m);
    };
    ws.onopen = () => {
      ws.send(JSON.stringify({ type: "register", role, id }));
      resolve(ws);
    };
  });
}
function next(ws, timeoutMs = 2000) {
  if (ws.inbox.length) return Promise.resolve(ws.inbox.shift());
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error(`${ws.id}: timeout waiting for message`)), timeoutMs);
    ws.waiters.push((m) => {
      clearTimeout(t);
      resolve(m);
    });
  });
}
async function drainRegistered(ws) {
  const m = await next(ws);
  if (m.type !== "registered") throw new Error(`${ws.id}: expected registered, got ${JSON.stringify(m)}`);
}
const ok = (msg) => console.log(`  ✓ ${msg}`);

async function run() {
  const ssoOn = process.env.SSO === "1";

  // ---- Open mode (no ARNA_SSO_SECRET): connect_request -> incoming_request ----
  if (!ssoOn) {
  console.log("[open mode]");
  const agent = await open("agent-smoke", "agent");
  const console_ = await open("viewer-smoke", "console");
  await drainRegistered(agent);
  await drainRegistered(console_);

  console_.send(JSON.stringify({ type: "connect_request", to: "agent-smoke" }));
  const req = await next(agent);
  if (req.type !== "incoming_request" || req.from !== "viewer-smoke")
    throw new Error("agent did not get incoming_request: " + JSON.stringify(req));
  ok(`agent received incoming_request from ${req.from} as "${req.name}"`);

  // agent accepts via relayed signal
  agent.send(JSON.stringify({ type: "signal", to: req.from, data: { kind: "consent", accepted: true, code: "123456" } }));
  const consent = await next(console_);
  if (consent.type !== "signal" || consent.data.kind !== "consent" || !consent.data.accepted)
    throw new Error("console did not get accept: " + JSON.stringify(consent));
  ok(`console received consent accepted, code ${consent.data.code}`);

  // request to an offline agent -> request_denied
  console_.send(JSON.stringify({ type: "connect_request", to: "nope" }));
  const denied = await next(console_);
  if (denied.type !== "request_denied") throw new Error("expected request_denied: " + JSON.stringify(denied));
  ok(`offline agent -> request_denied: "${denied.reason}"`);

  }

  // ---- SSO mode (requires backend started with ARNA_SSO_SECRET + ARNA_DEV_TICKETS=1) ----
  if (ssoOn) {
    console.log("[sso mode]");
    const a2 = await open("agent-sso", "agent");
    const c2 = await open("viewer-sso", "console");
    await drainRegistered(a2);
    await drainRegistered(c2);

    // no ticket -> denied
    c2.send(JSON.stringify({ type: "connect_request", to: "agent-sso" }));
    const d2 = await next(c2);
    if (d2.type !== "request_denied") throw new Error("no-ticket should be denied: " + JSON.stringify(d2));
    ok(`no ticket -> request_denied: "${d2.reason}"`);

    // mint a dev ticket and retry -> incoming_request with the ticket's name
    const res = await fetch(`${HTTP}/dev/ticket?agent=agent-sso&name=Tarun`);
    const { ticket } = await res.json();
    if (!ticket) throw new Error("dev ticket endpoint returned no ticket");
    ok("minted dev ticket");
    c2.send(JSON.stringify({ type: "connect_request", to: "agent-sso", ticket }));
    const r2 = await next(a2);
    if (r2.type !== "incoming_request" || r2.name !== "Tarun")
      throw new Error("valid ticket should route with name: " + JSON.stringify(r2));
    ok(`valid ticket -> incoming_request as "${r2.name}"`);

    // wrong agent in ticket -> denied
    const res2 = await fetch(`${HTTP}/dev/ticket?agent=other-agent&name=Tarun`);
    const { ticket: wrong } = await res2.json();
    c2.send(JSON.stringify({ type: "connect_request", to: "agent-sso", ticket: wrong }));
    const d3 = await next(c2);
    if (d3.type !== "request_denied") throw new Error("ticket pinned to other agent should be denied: " + JSON.stringify(d3));
    ok(`ticket pinned to other agent -> request_denied: "${d3.reason}"`);

  } else {
    console.log("[sso mode] skipped (set SSO=1 with an SSO-enabled backend)");
  }

  console.log("\nALL CHECKS PASSED");
}

// Detach handlers and close sockets, then exit on the next tick — avoids a libuv
// "UV_HANDLE_CLOSING" assertion when process.exit races an in-flight WS close.
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
    console.error("FAILED:", e.message);
    shutdown(1);
  },
);
