#!/usr/bin/env bash
# Build an immutable wse-linux-x11 runtime image (display stack + catalog apps).
#
# Produces dist/wse-linux-x11-v<VER>.tar and stamps its sha256 into the manifest.
# The image is IMMUTABLE: never edit a built image; bump the version and rebuild.
# Runs on the Windows host and drives wsl.exe. See ./README.md.
set -euo pipefail

VER="${1:-1.0.0}"
NAME="wse-linux-x11"
BUILDER="wse-rt-builder-$$"                       # throwaway builder distro
HERE="$(cd "$(dirname "$0")" && pwd)"
DIST="$HERE/dist"; mkdir -p "$DIST"
ROOTFS_URL="https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/x86_64/alpine-minirootfs-3.20.3-x86_64.tar.gz"
ROOTFS="$DIST/alpine-minirootfs.tar.gz"
OUT="$DIST/$NAME-v$VER.tar"

cleanup() { wsl.exe --unregister "$BUILDER" >/dev/null 2>&1 || true; }
trap cleanup EXIT

[ -f "$ROOTFS" ] || curl.exe -fsSL "$ROOTFS_URL" -o "$ROOTFS"

# 1. Import a builder from the bare rootfs.
BROOT="$DIST/$BUILDER"; mkdir -p "$BROOT"
wsl.exe --import "$BUILDER" "$BROOT" "$ROOTFS"

# 2. Install the immutable display stack + catalog apps, and bake the in-workspace
#    manifest. Everything the runtime provides is fixed here, once.
wsl.exe -d "$BUILDER" -- sh -euxc '
  apk update
  apk add --no-cache \
    xvfb openbox xterm wmctrl xdotool \
    ttf-dejavu font-noto \
    netsurf-gtk vim
  mkdir -p /opt/wse
  # The runtime advertises itself from inside, too (defence in depth).
  cat > /opt/wse/runtime.json <<JSON
{ "id":"'"$NAME"'", "version":"'"$VER"'", "display":":0", "wm":"openbox" }
JSON
  # Boot script the adapter invokes to bring up the display before launching apps.
  cat > /opt/wse/start-display.sh <<SH
#!/bin/sh
export DISPLAY=:0
Xvfb :0 -screen 0 1920x1080x24 >/dev/null 2>&1 &
sleep 1
openbox >/dev/null 2>&1 &
SH
  chmod +x /opt/wse/start-display.sh
'

# 3. Export to an immutable tar and content-address it.
wsl.exe --export "$BUILDER" "$OUT"
DIGEST="sha256:$(sha256sum "$OUT" | cut -d" " -f1)"
echo "built $OUT"
echo "digest $DIGEST"

# 4. Stamp the digest into the manifest (the only mutable record; the image is not).
MAN="$HERE/manifest.v$( echo "$VER" | cut -d. -f1 ).json"
if [ -f "$MAN" ]; then
  sed -i "s#\"digest\": \"[^\"]*\"#\"digest\": \"$DIGEST\"#" "$MAN"
  echo "stamped $MAN"
fi
