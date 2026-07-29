# Runtime: `wse-linux-x11`

The Linux/X11 execution environment WSE workspaces run on today. An **immutable,
versioned image**, not a setup script — see [contract/core/runtime.md](../../contract/core/runtime.md).

The adapter *imports and launches* this image; it never provisions inside a
running workspace. That separation (platform orchestration vs. workspace
execution) is the whole point: the same runtime concepts will back the Linux,
macOS, and cloud adapters, so none of them re-embed provisioning logic.

## What's inside

| Layer | Provides |
|-------|----------|
| Alpine userspace | base OS |
| Xvfb | a headless X display |
| openbox | window manager (real windows, focus, stacking) |
| wmctrl / xdotool | window discovery + input (the Windows & Applications capabilities) |
| ttf-dejavu | fonts (so app windows actually render — see the xterm/musl lesson) |
| catalog apps | the applications a workspace may launch (see `manifest.v1.json`) |

Together these make the runtime **provide the Applications and Windows
capabilities** — which is why bumping to v1.0.0 flips those on (adapter ∩ runtime).

## Versions

Runtime images are immutable. A change is a **new version + new digest**, never an
in-place edit.

| Version | Digest | Capabilities | Notes |
|---------|--------|--------------|-------|
| v0.1.0  | `alpine-minirootfs-3.20.3-x86_64` | (none) | bare rootfs; what the adapter ships today |
| v1.0.0  | (produced by `build.sh`) | Applications, Windows | first display-capable image |

## Building v1

```sh
# On the Windows host (drives wsl.exe); produces the immutable image + manifest.
./build.sh 1.0.0
```

`build.sh` builds the image in a throwaway builder distro, exports it to
`dist/wse-linux-x11-v1.0.0.tar`, computes its sha256 `digest`, and writes
`manifest.v1.json`. The `.tar` is the immutable artifact — it lives **outside
git** (large, content-addressed); only the recipe and manifest are tracked.

## Wiring it into the adapter

Once built, the Windows adapter's `runtime()` returns v1.0.0's descriptor (name,
version, digest from the manifest, `capabilities` = Applications + Windows) and
`create()` imports `dist/wse-linux-x11-v1.0.0.tar` instead of the bare rootfs.
`ApplicationsCapability` then maps a catalog `entry` onto the manifest's command,
launches it on the Xvfb display, and reads the window back via `wmctrl`. That is
what turns `run_applications` green on a real OS.
