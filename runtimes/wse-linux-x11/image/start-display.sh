#!/bin/sh
# Bring up the headless display + window manager. Idempotent: the adapter runs
# this before launching apps, and it must survive across wsl.exe invocations.
export DISPLAY=:0
pgrep -x Xvfb >/dev/null || (Xvfb :0 -screen 0 1920x1080x24 >/tmp/xvfb.log 2>&1 &)
for i in $(seq 1 60); do
  xdotool getdisplaygeometry >/dev/null 2>&1 && break
  sleep 0.2
done
pgrep -x openbox >/dev/null || (openbox >/tmp/openbox.log 2>&1 &)
sleep 0.3
