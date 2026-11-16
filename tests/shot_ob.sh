#!/bin/sh
# Captures de l'assistant de premier lancement, À LANCER DANS LE CONTENEUR :
#   build/remote.sh ob 'cd crates/plume-app/src-tauri && cargo build --release -p plume-app >/dev/null && cd /src && sh tests/shot_ob.sh'
#
# Même préambule que tests/ui_shot.sh (Xvfb, session D-Bus, WebKit logiciel).
# Une capture par état : PLUME_DEMO=step=N (voir js/onboarding.js), thème clair
# ou sombre. Les images vont dans docs/shots/ob/, recadrées sur la fenêtre
# elle-même : 560 × 540 centrée dans l'écran 1280 × 800, donc à (360, 130).
set -eu

if [ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ] && command -v dbus-run-session >/dev/null 2>&1; then
  exec dbus-run-session -- "$0" "$@"
fi

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
BIN=${PLUME_BIN:-$ROOT/target-linux/release/plume}
SHOTS=${PLUME_SHOTS:-$ROOT/docs/shots/ob}
DATA=/tmp/plume-ob
DISPLAY_NUMBER=99

if [ ! -x "$BIN" ]; then
  echo "binaire absent : $BIN (lancer cargo build --release -p plume-app)" >&2
  exit 1
fi

mkdir -p "$SHOTS"
rm -f "$SHOTS"/*.png

XVFB_PID=""
if ! xdpyinfo -display ":$DISPLAY_NUMBER" >/dev/null 2>&1; then
  Xvfb ":$DISPLAY_NUMBER" -screen 0 1280x800x24 >/tmp/plume-xvfb.log 2>&1 &
  XVFB_PID=$!
  tries=0
  while ! xdpyinfo -display ":$DISPLAY_NUMBER" >/dev/null 2>&1; do
    tries=$((tries + 1))
    if [ "$tries" -gt 50 ]; then
      echo "Xvfb ne répond pas (voir /tmp/plume-xvfb.log)" >&2
      exit 1
    fi
    sleep 0.2
  done
fi

export DISPLAY=":$DISPLAY_NUMBER"
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export LIBGL_ALWAYS_SOFTWARE=1
export GDK_BACKEND=x11
export XDG_RUNTIME_DIR=/tmp/plume-runtime
export NO_AT_BRIDGE=1
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

APP_PID=""
cleanup() {
  if [ -n "$APP_PID" ]; then
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
  if [ -n "$XVFB_PID" ]; then
    kill "$XVFB_PID" 2>/dev/null || true
    wait "$XVFB_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

# shot <nom> <demo> <thème>
shot() {
  name=$1
  demo=$2
  theme=$3

  rm -rf "$DATA"
  PLUME_DATA_DIR="$DATA" PLUME_SHOW=onboarding PLUME_DEMO="$demo" PLUME_THEME="$theme" \
    "$BIN" >"/tmp/plume-$name.log" 2>&1 &
  APP_PID=$!

  sleep 4
  import -window root -define png:compression-level=3 "png:-" 2>"/tmp/plume-$name-import.log" |
    convert - -crop 560x540+360+130 +repage "$SHOTS/$name.png" 2>>"/tmp/plume-$name-import.log" ||
    import -window root "$SHOTS/$name.png" 2>>"/tmp/plume-$name-import.log" || true

  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=""
}

shot ob-1-welcome-light step=1 light
shot ob-1-welcome-dark step=1 dark
shot ob-2-microphone-light step=2 light
shot ob-3-paste-light step=3 light
shot ob-3-paste-missing-light step=3-missing light
shot ob-3-paste-wayland-light step=3-wayland light
shot ob-4-shortcut-light step=4 light
shot ob-5-language-light step=5 light
shot ob-6-model-light step=6 light
shot ob-6-model-dark step=6 dark
shot ob-6-model-downloading-light step=6-downloading light
shot ob-7-cleanup-light step=7 light
shot ob-7-cleanup-dark step=7 dark
shot ob-8-done-light step=8 light
shot ob-8-done-dark step=8 dark

# Contrôle de navigation : un clic sur « Continue » doit passer à l'étape 2
# (elle est reconnue à son bouton « Re-check » et à son compteur de points).
rm -rf "$DATA"
PLUME_DATA_DIR="$DATA" PLUME_SHOW=onboarding PLUME_DEMO=step=1 PLUME_THEME=light \
  "$BIN" >/tmp/plume-nav.log 2>&1 &
APP_PID=$!
sleep 4
xdotool mousemove 850 634 click 1 2>/dev/null || true
sleep 2
import -window root "png:-" 2>/tmp/plume-nav-import.log |
  convert - -crop 560x540+360+130 +repage "$SHOTS/ob-nav-step2.png" 2>>/tmp/plume-nav-import.log ||
  import -window root "$SHOTS/ob-nav-step2.png" 2>>/tmp/plume-nav-import.log || true
kill "$APP_PID" 2>/dev/null || true
wait "$APP_PID" 2>/dev/null || true
APP_PID=""

status=0
count=0
for file in "$SHOTS"/*.png; do
  [ -s "$file" ] || continue
  count=$((count + 1))
  size=$(wc -c <"$file")
  if [ "$size" -lt 4096 ]; then
    echo "capture trop petite : $file ($size octets)" >&2
    status=1
  fi
done
echo "$count captures dans $SHOTS"
[ "$count" -ge 10 ] || status=1
exit "$status"
