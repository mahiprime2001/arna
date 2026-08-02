#!/bin/sh
# /opt/wse/clip.sh set <mime>   (payload as base64 on stdin)
# /opt/wse/clip.sh get          (prints "<mime>\n<base64>"; empty if none)
#
# The runtime's CLIPBOARD SERVICE. The workspace clipboard's authoritative content
# is stored in the runtime (so ownership survives across the adapter's separate
# invocations — a bare xclip holder does not, it dies with its wsl session), and
# is ALSO published to the real X11 CLIPBOARD selection via a detached xclip so
# apps inside the workspace can paste it. The service knows nothing about WSE
# policy, roles, or events — those are the engine's.
export DISPLAY=:0
DIR=/tmp/wse-clip
MIME="$DIR/mime"
BIN="$DIR/bin"
mkdir -p "$DIR"

case "$1" in
  set)
    mime="$2"
    cat | base64 -d > "$BIN"            # base64 payload on stdin -> raw bytes
    printf '%s' "$mime" > "$MIME"
    # Publish to the real X clipboard, detached so it outlives this wsl session
    # and holds the selection for apps in the workspace.
    setsid sh -c "xclip -selection clipboard -t '$mime' -i < '$BIN'" >/dev/null 2>&1 &
    ;;
  get)
    [ -f "$MIME" ] || exit 0            # nothing owns the clipboard
    mime=$(cat "$MIME")
    printf '%s\n' "$mime"
    # Prefer the live X11 selection (so an app's copy is seen); fall back to the
    # durable store when no holder is alive (WSL reaps session processes).
    live=$(xclip -selection clipboard -o -t "$mime" 2>/dev/null)
    if [ -n "$live" ]; then
      printf '%s' "$live" | base64
    else
      base64 < "$BIN"
    fi
    ;;
  *)
    echo "usage: clip.sh set <mime> | get" >&2
    exit 2
    ;;
esac
