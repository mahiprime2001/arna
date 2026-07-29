#!/bin/sh
# /opt/wse/launch.sh <entry> <marker>
# Launch a catalog app (from apps.conf) with a unique window title = <marker>,
# then print the launched window's X id on stdout. The adapter never learns which
# command an entry maps to — that lives here, in the runtime.
export DISPLAY=:0
entry="$1"
marker="$2"

line=$(grep "^${entry}|" /opt/wse/apps.conf | head -1)
if [ -z "$line" ]; then
  echo "no such app: $entry" >&2
  exit 44
fi

cmd=$(printf '%s' "$line" | cut -d'|' -f2- | sed "s/__TITLE__/${marker}/g")
setsid sh -c "$cmd" >/dev/null 2>&1 &

# Wait for the window with our exact marker title, activate it (deterministic
# focus = newest), and return its id.
i=0
while [ "$i" -lt 60 ]; do
  wid=$(xdotool search --onlyvisible --name "^${marker}\$" 2>/dev/null | head -1)
  if [ -n "$wid" ]; then
    xdotool windowactivate --sync "$wid" >/dev/null 2>&1 || true
    echo "$wid"
    exit 0
  fi
  i=$((i + 1))
  sleep 0.2
done

echo "window for $entry ($marker) did not appear" >&2
exit 45
