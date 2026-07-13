# Arna login/signaling server on a VPS (IP:port)

The minimal deploy: one Docker container running the Rust backend. It handles
**accounts (login)** and the tiny **connection introduction (signaling)**. Your
screen/keyboard/files stay **peer-to-peer** and never pass through this server.

## Deploy

On the VPS (Docker + Docker Compose installed), from this folder:

```bash
cp .env.example .env
# edit .env → set ARNA_SSO_SECRET to a long random value:
#   openssl rand -hex 32
docker compose up -d --build
```

Open the port on the firewall:

```bash
ufw allow 48080/tcp        # or your ARNA_PORT
# (or add it in your cloud provider's security group)
```

Open the console port too (default 48090):

```bash
ufw allow 48090/tcp
```

Check both are up:

```bash
curl http://<vps-ip>:48080/health           # server -> ok
# open http://<vps-ip>:48090/ in a browser  # the console web app
```

## Use it (no install)

This deploys **two** things: the server (accounts + signaling) and the
**console web app**. So anyone can just:

1. Open **`http://<vps-ip>:48090/`** in a browser (Chrome).
2. **Sign up / sign in** — the server is already baked in, no URL to type.
3. To make a Windows PC reachable, run the desktop app there and sign in with
   the same account — it registers itself automatically. Then it shows up in the
   browser's device list; click to connect.

The desktop installers (built by the `desktop` GitHub Action) are baked to the
same server, so they connect automatically too.

## Manage

```bash
docker compose logs -f arna-server     # logs
docker compose restart arna-server     # restart (accounts persist)
docker compose down                    # stop (keeps the arna_data volume)
docker compose down -v                 # stop AND wipe accounts
```

Accounts live in the `arna_data` volume (`/data/arna.db`), so they survive
restarts and rebuilds.

## ⚠️ Before real use: add TLS

Over plain `ws://` + `http://` on an IP, **login passwords travel unencrypted**.
Fine for testing on your own network / a throwaway VPS, but for real use put a
domain in front with HTTPS/`wss://` — the full stack in
[`../docker-compose.yml`](../docker-compose.yml) does this with Caddy
(auto-HTTPS) and adds a coturn relay. Then apps use `wss://your-domain/ws`.
