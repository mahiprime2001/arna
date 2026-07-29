#!/usr/bin/env bash
# Build wse-lite: a deliberately MINIMAL, headless runtime. It provides NO
# capabilities inside the workspace — no display, no clipboard, no storage. Its
# whole purpose is to prove runtime interchangeability: the *same* adapter, run on
# this runtime instead of wse-linux-x11, negotiates effective = adapter ∩ runtime
# and naturally reports capabilities as unavailable. See ../wse-linux-x11 and
# contract/core/runtime.md.
set -euo pipefail

VER="${1:-1.0.0}"
NAME="wse-lite"
BUILDER="wse-lite-builder-$$"
HERE="$(cd "$(dirname "$0")" && pwd)"
DIST="$HERE/dist"; mkdir -p "$DIST"
ALPINE="v3.20"
ROOTFS_URL="https://dl-cdn.alpinelinux.org/alpine/$ALPINE/releases/x86_64/alpine-minirootfs-3.20.3-x86_64.tar.gz"
ROOTFS="$DIST/alpine-minirootfs.tar.gz"
OUT="$DIST/$NAME-v$VER.tar"

cleanup() { wsl.exe --unregister "$BUILDER" >/dev/null 2>&1 || true; }
trap cleanup EXIT

[ -f "$ROOTFS" ] || curl.exe -fsSL "$ROOTFS_URL" -o "$ROOTFS"

BROOT="$DIST/$BUILDER"; mkdir -p "$BROOT"
wsl.exe --import "$BUILDER" "$BROOT" "$ROOTFS"

# The only thing baked in is the runtime's self-description. Nothing else — that
# is the point. No packages, no display stack.
wsl.exe -d "$BUILDER" -- sh -euxc "
  mkdir -p /opt/wse
  printf '{ \"id\":\"$NAME\", \"version\":\"$VER\", \"capabilities\":[] }\n' > /opt/wse/runtime.json
"

wsl.exe --export "$BUILDER" "$OUT"
DIGEST="sha256:$(sha256sum "$OUT" | cut -d' ' -f1)"
echo "built $OUT"
echo "digest $DIGEST"

MAJOR="$(echo "$VER" | cut -d. -f1)"
MAN="$HERE/manifest.v$MAJOR.json"
if [ -f "$MAN" ]; then
  sed -i "s#\"digest\": \"[^\"]*\"#\"digest\": \"$DIGEST\"#" "$MAN"
  echo "stamped $MAN"
fi
