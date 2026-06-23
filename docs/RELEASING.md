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

## Desktop apps (Agent + Console)

Installer builds (Windows / macOS / Linux) are wired into `release.yml` as the
commented `desktop-apps` job. Uncomment it once `agent/` and `console/` are real
Tauri projects; `tauri-action` will attach the installers to the same Release.

## Versioning

Semantic versioning: `vMAJOR.MINOR.PATCH`. Tags must match `v*.*.*`.
