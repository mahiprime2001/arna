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
IMG="$HERE/image"                                 # tracked in-workspace scripts
ALPINE="v3.20"
ROOTFS_URL="https://dl-cdn.alpinelinux.org/alpine/$ALPINE/releases/x86_64/alpine-minirootfs-3.20.3-x86_64.tar.gz"
ROOTFS="$DIST/alpine-minirootfs.tar.gz"
OUT="$DIST/$NAME-v$VER.tar"

cleanup() { wsl.exe --unregister "$BUILDER" >/dev/null 2>&1 || true; }
trap cleanup EXIT

[ -f "$ROOTFS" ] || curl.exe -fsSL "$ROOTFS_URL" -o "$ROOTFS"

# 1. Import a builder from the bare rootfs.
BROOT="$DIST/$BUILDER"; mkdir -p "$BROOT"
wsl.exe --import "$BUILDER" "$BROOT" "$ROOTFS"

# 2. Install the immutable display stack. The X tools live in Alpine's community
#    repo, which the minirootfs ships disabled. (No wmctrl — Alpine doesn't
#    package it; xdotool covers discovery + focus. See ENGINEERING_LOG #1.)
wsl.exe -d "$BUILDER" -- sh -euxc "
  echo 'https://dl-cdn.alpinelinux.org/alpine/$ALPINE/community' >> /etc/apk/repositories
  apk update
  apk add --no-cache xvfb openbox xterm xdotool ttf-dejavu
  mkdir -p /opt/wse
  printf '{ \"id\":\"$NAME\", \"version\":\"$VER\" }\n' > /opt/wse/runtime.json
"

# 3. Install the runtime's own scripts (tracked recipe files, piped in — no
#    heredoc quoting games, and reviewable in git).
for f in start-display.sh launch.sh apps.conf; do
  wsl.exe -d "$BUILDER" -- sh -c "cat > /opt/wse/$f" < "$IMG/$f"
done
wsl.exe -d "$BUILDER" -- sh -c "chmod +x /opt/wse/start-display.sh /opt/wse/launch.sh"

# 4. Export to an immutable tar and content-address it.
wsl.exe --export "$BUILDER" "$OUT"
DIGEST="sha256:$(sha256sum "$OUT" | cut -d' ' -f1)"
echo "built $OUT"
echo "digest $DIGEST"

# 5. Stamp the digest into the manifest (the only mutable record; image is not).
MAJOR="$(echo "$VER" | cut -d. -f1)"
MAN="$HERE/manifest.v$MAJOR.json"
if [ -f "$MAN" ]; then
  sed -i "s#\"digest\": \"[^\"]*\"#\"digest\": \"$DIGEST\"#" "$MAN"
  echo "stamped $MAN"
fi
