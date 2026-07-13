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

Check it's up:

```bash
curl http://localhost:48080/health          # -> ok
curl http://<vps-ip>:48080/health           # from your machine
```

## Point the apps at it

- **Console (browser/app):** on the sign-in screen open **Server** and set
  `ws://<vps-ip>:48080/ws`, then sign up / log in.
- **Bake it into a build** so users don't type it — build the app with
  `VITE_ARNA_BACKEND=ws://<vps-ip>:48080/ws` (and `ARNA_DEFAULT_BACKEND=...` for
  the agent). See [`../../docs/RELEASING.md`](../../docs/RELEASING.md).

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
