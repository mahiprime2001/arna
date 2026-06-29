# Releasing Arna

Releases are **tag-driven** via [`.github/workflows/release.yml`](../.github/workflows/release.yml).

## Cut a release

```bash
git tag v0.1.0
git push origin v0.1.0
```

The `release` workflow then:

1. **Builds the backend** for `x86_64-unknown-linux-gnu` and `x86_64-unknown-linux-musl`
   (static), packaged as `arna-backend-<tag>-<target>.tar.gz`.
2. **Pushes the backend Docker image** to GHCR:
   `ghcr.io/mahiprime2001/arna-backend:<version>` (+ `:latest` for stable tags).
3. **Publishes a GitHub Release** with auto-generated notes, the binary archives,
   and a `SHA256SUMS.txt`.

## Pre-releases

Use a tag containing a hyphen — e.g. `v0.1.0-beta.1`. The release is marked as a
**pre-release** and the Docker image is **not** tagged `latest`.

## Desktop app (Arna)

There's **one** app — `console/` builds the unified "Arna" (console UI + the
agent loop, so it both controls others and can be controlled). Build installers
locally with `tauri build`:

```bash
cd console && npm install && npm run tauri:build
```

On Windows this produces, under `console/src-tauri/target/release/bundle/`:
- `msi/Arna_<ver>_x64_en-US.msi` (WiX)
- `nsis/Arna_<ver>_x64-setup.exe` (NSIS)

(`agent-desktop/` is legacy and no longer shipped; the headless `agent` binary
is still built for unattended/CI use.)

(macOS → `.dmg`/`.app`; Linux → `.AppImage`/`.deb` when built on those OSes.)

### Point a build at your hosted server

By default a build talks to `ws://127.0.0.1:8081/ws` (LAN/dev). The server is
also editable in-app (the console remembers it; the agent's pairing window has a
Server field), so unconfigured installers still work — users just type the URL.
To bake your hosted backend into a build so it "just works", set both **at build
time** (in `console/`): `VITE_ARNA_BACKEND` for the console UI and
`ARNA_DEFAULT_BACKEND` for the agent loop (compile-time `option_env!`):

```bash
cd console
VITE_ARNA_BACKEND=wss://api.your-domain.com/ws \
ARNA_DEFAULT_BACKEND=wss://api.your-domain.com/ws \
  npm run tauri:build
```

Pair this with a deployed backend that has TURN configured (`ARNA_TURN*`, see
[SECURITY.md](SECURITY.md) deploy step 6) for reliable cross-internet sessions.

### CI

The commented `desktop-apps` job in `release.yml` uses `tauri-action` to build
and attach these installers to the same GitHub Release — uncomment it to ship
installers per tag. Code-signing is not set up yet, so installers are unsigned
(Windows SmartScreen will warn until signed).

## Versioning

Semantic versioning: `vMAJOR.MINOR.PATCH`. Tags must match `v*.*.*`.
