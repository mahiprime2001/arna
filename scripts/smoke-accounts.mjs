// Accounts foundation smoke test: signup / login / duplicate / wrong password.
// Run against a backend started with ARNA_SSO_SECRET set (accounts need it):
//   ARNA_SSO_SECRET=dev ARNA_DB=:memory: ./arna-backend.exe
//   node scripts/smoke-accounts.mjs

const HTTP = "http://127.0.0.1:8081";
const email = `t${Date.now()}@example.com`;
const password = "hunter2hunter2";

const ok = (m) => console.log(`  ✓ ${m}`);
const expect = (c, m) => {
  if (!c) throw new Error("FAILED: " + m);
};
async function post(path, body) {
  const r = await fetch(HTTP + path, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  let json = null;
  try {
    json = await r.json();
  } catch {}
  return { status: r.status, json };
}

async function run() {
  // 1) Signup -> token
  let r = await post("/auth/signup", { email, password });
  expect(r.status === 200 && r.json?.token, `signup should return a token (got ${r.status})`);
  expect(r.json.token.split(".").length === 3, "token should be a JWT");
  const userId = r.json.user_id;
  ok(`signup creates an account (user ${userId}, token issued)`);

  // 2) Duplicate email -> 409
  r = await post("/auth/signup", { email, password });
  expect(r.status === 409, `duplicate email should be 409 (got ${r.status})`);
  ok("duplicate email is rejected (409)");

  // 3) Short password -> 400
  r = await post("/auth/signup", { email: `x${Date.now()}@example.com`, password: "short" });
  expect(r.status === 400, `short password should be 400 (got ${r.status})`);
  ok("short password is rejected (400)");

  // 4) Login with correct password -> token, same user
  r = await post("/auth/login", { email, password });
  expect(r.status === 200 && r.json?.token, `login should return a token (got ${r.status})`);
  expect(r.json.user_id === userId, "login returns the same user id");
  ok("login with correct password succeeds");

  // 5) Wrong password -> 401
  r = await post("/auth/login", { email, password: "wrongwrongwrong" });
  expect(r.status === 401, `wrong password should be 401 (got ${r.status})`);
  ok("wrong password is rejected (401)");

  // 6) Unknown email -> 401 (same vague error)
  r = await post("/auth/login", { email: "nobody@example.com", password });
  expect(r.status === 401, `unknown email should be 401 (got ${r.status})`);
  ok("unknown email is rejected (401)");

  console.log("\nACCOUNTS OK");
}
run().then(
  () => process.exit(0),
  (e) => {
    console.error(e.message);
    process.exit(1);
  },
);
