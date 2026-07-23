# Dev run scripts

## Recommended: one port + ngrok

Everything (app, `/api`, `/ws`) is served from **one port**, so **one tunnel**
covers the whole app. ngrok puts trusted HTTPS in front, which means camera/mic
work on every device with **no certs and no Chrome flags**.

One-time ngrok setup:

```powershell
winget install ngrok.ngrok          # or download from https://ngrok.com/download
ngrok config add-authtoken <your-token>   # free account -> dashboard
```

Then, two terminals:

```powershell
# 1) build the client + serve app/API/WS on :8787
scripts\serve.ps1

# 2) tunnel that one port
ngrok http 8787
```

ngrok prints a `https://<something>.ngrok-free.app` URL. Open it on any device —
sign up, add friends, chat, call. Done.

Notes:
- A **free ngrok account includes one static domain**, so the URL stops changing:
  grab it from the dashboard (Domains) and run
  `ngrok http 8787 --url=<your-name>.ngrok-free.app`.
- `scripts\serve.ps1 -NoBuild` skips the client rebuild. After editing client
  code, re-run without `-NoBuild` (the served files are the built ones).
- The backend stays plain HTTP on purpose — the tunnel terminates TLS.
- Calls between devices on **different** networks may still need a TURN server
  for the P2P media (the tunnel only carries signaling). Same-network is fine.

---

The rest below are alternatives; you don't need them if the above works.

## Plain HTTP (localhost only)

```powershell
# terminal 1
cd services ; go run .
# terminal 2
cd client ; npm run dev
```

Open `http://localhost:4320`. Camera/mic (calls) work on `localhost` but NOT when
another device opens it via a LAN IP over http (browsers block mic/camera on
insecure origins).

## HTTPS (calls work between devices on the LAN, no Chrome flag)

One-time cert setup (host PC):

```powershell
go install filippo.io/mkcert@latest
& "$env:USERPROFILE\go\bin\mkcert.exe" -install
mkdir infra\certs -Force
& "$env:USERPROFILE\go\bin\mkcert.exe" -cert-file infra\certs\dev-cert.pem -key-file infra\certs\dev-key.pem <this-pc-lan-ip> localhost 127.0.0.1
```

Then run each in its own terminal:

```powershell
scripts\backend-https.ps1     # https/wss on :8787
scripts\client-https.ps1      # https on :4320
```

Open `https://<this-pc-lan-ip>:4320`.

### Trust the cert on OTHER devices (one-time each)

Copy the root CA from the host to the other device and install it:

- Host copy of the root CA: `infra\certs\rootCA.pem` (or `%LOCALAPPDATA%\mkcert\rootCA.pem`).
- On the other device: double-click `rootCA.pem` -> Install Certificate ->
  Current User -> "Place all certificates in the following store" ->
  **Trusted Root Certification Authorities**. (Or, in an admin prompt:
  `certutil -addstore -f Root rootCA.pem`.)

After that the other device opens `https://<host-ip>:4320` with a green lock and
calls work with no browser flag.

> Certs in `infra/certs/` are machine-specific and gitignored. Regenerate them
> per the one-time setup above on each host.

## Tunnel the dev server instead (keeps hot reload)

Same idea as the recommended setup, but tunnelling vite instead of the backend,
so client edits hot-reload while you test on other devices. Costs an extra
process; vite proxies `/api` + `/ws` to the backend so one tunnel still covers
everything. Use `ngrok http 4320` or:

```powershell
# 1) backend (plain http is fine; the tunnel provides https)
cd services ; $env:ARNA_DB="arna-social.db" ; go run .
# 2) client (plain http; proxies /api + /ws to the backend)
cd client ; npm run dev
# 3) tunnel the client origin
cloudflared tunnel --url http://localhost:4320
```

`cloudflared` prints a `https://<random>.trycloudflare.com` URL. Open that on any
device — camera/mic work (trusted https), no flag needed. Get cloudflared from
<https://github.com/cloudflare/cloudflared/releases> (no account needed for quick
tunnels). ngrok works too (`ngrok http 4320`) but needs a free authtoken.

Notes:
- The quick-tunnel URL changes each restart. A stable URL needs a Cloudflare
  account + named tunnel (or an ngrok reserved domain).
- Calls between devices on **different** networks may still need a TURN server for
  the P2P media (the tunnel only carries signaling). Same-network calls are fine.
