#!/bin/sh
# Captures de la coquille, À LANCER DANS LE CONTENEUR DE BUILD :
#   build/remote.sh app 'sh tests/ui_shot.sh'
#
# Xvfb 1280×800, session D-Bus si elle manque, binaire release, 6 s par capture,
# puis import -window root. Trois images : main (clair), onboarding, main-dark
# (PLUME_THEME=dark). Sort en 1 si une image manque ou fait moins de 20 Ko.
set -eu

# Le bus de session (instance unique, zone de notification) : on relance le
# script dedans quand il n'y en a pas encore.
if [ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ] && command -v dbus-run-session >/dev/null 2>&1; then
  exec dbus-run-session -- "$0" "$@"
fi

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
BIN=${PLUME_BIN:-$ROOT/target-linux/release/plume}
SHOTS=${PLUME_SHOTS:-$ROOT/docs/shots}
DATA=/tmp/plume-shot
DISPLAY_NUMBER=99

if [ ! -x "$BIN" ]; then
  echo "binaire absent : $BIN (lancer cargo tauri build avant)" >&2
  exit 1
fi

mkdir -p "$SHOTS"

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
# Pas de GPU dans le conteneur : WebKit en rendu logiciel.
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

# shot <nom> <PLUME_SHOW> [thème]
shot() {
  name=$1
  show=$2
  theme=${3:-}

  rm -rf "$DATA"
  if [ -n "$theme" ]; then
    PLUME_DATA_DIR="$DATA" PLUME_SHOW="$show" PLUME_THEME="$theme" \
      "$BIN" >"/tmp/plume-$name.log" 2>&1 &
  else
    PLUME_DATA_DIR="$DATA" PLUME_SHOW="$show" \
      "$BIN" >"/tmp/plume-$name.log" 2>&1 &
  fi
  APP_PID=$!

  sleep 6
  import -window root -define png:compression-level=3 "$SHOTS/$name.png" \
    2>"/tmp/plume-$name-import.log" || true

  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=""
}

shot main main
shot onboarding onboarding
shot main-dark main dark

status=0
for name in main onboarding main-dark; do
  file="$SHOTS/$name.png"
  if [ ! -s "$file" ]; then
    echo "capture manquante : $file" >&2
    status=1
    continue
  fi
  size=$(wc -c <"$file")
  if [ "$size" -lt 20480 ]; then
    echo "capture trop petite : $file ($size octets)" >&2
    status=1
  else
    echo "$file ($size octets)"
  fi
done

exit "$status"
