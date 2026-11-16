#!/bin/sh
# Captures des pages Accueil / Historique / Vocabulaire, À LANCER DANS LE
# CONTENEUR DE BUILD :
#   build/remote.sh pa 'cd crates/plume-app/src-tauri && cargo build --release -p plume-app >/dev/null && cd /src && sh tests/shot_pa.sh'
#
# Même préambule que tests/ui_shot.sh (Xvfb 1280×800, session D-Bus, rendu
# logiciel). Chaque état est joué par PLUME_DEMO (`page=...`), dans le thème
# demandé, puis capturé avec `import -window root`.
#
# Usage : sh tests/shot_pa.sh [nom-de-capture...]
#   sans argument : les neuf captures ; sinon seulement celles nommées.
set -eu

# Le bus de session (instance unique, zone de notification) : on relance le
# script dedans quand il n'y en a pas encore.
if [ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ] && command -v dbus-run-session >/dev/null 2>&1; then
  exec dbus-run-session -- "$0" "$@"
fi

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
BIN=${PLUME_BIN:-$ROOT/target-linux/release/plume}
SHOTS=${PLUME_SHOTS:-$ROOT/docs/shots/pa}
DATA=/tmp/plume-pa
DISPLAY_NUMBER=99

if [ ! -x "$BIN" ]; then
  echo "binaire absent : $BIN (lancer cargo build --release -p plume-app)" >&2
  exit 1
fi

mkdir -p "$SHOTS"

XVFB_PID=""
if ! xdpyinfo -display ":$DISPLAY_NUMBER" >/dev/null 2>&1; then
  Xvfb ":$DISPLAY_NUMBER" -screen 0 1280x800x24 >/tmp/plume-pa-xvfb.log 2>&1 &
  XVFB_PID=$!
  tries=0
  while ! xdpyinfo -display ":$DISPLAY_NUMBER" >/dev/null 2>&1; do
    tries=$((tries + 1))
    if [ "$tries" -gt 50 ]; then
      echo "Xvfb ne répond pas (voir /tmp/plume-pa-xvfb.log)" >&2
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

# shot <nom> <démo> <thème>
shot() {
  name=$1
  demo=$2
  theme=$3

  rm -rf "$DATA"
  PLUME_DATA_DIR="$DATA" PLUME_SHOW=main PLUME_DEMO="$demo" PLUME_THEME="$theme" \
    "$BIN" >"/tmp/plume-pa-$name.log" 2>&1 &
  APP_PID=$!

  sleep 6
  import -window root -define png:compression-level=3 "$SHOTS/$name.png" \
    2>"/tmp/plume-pa-$name-import.log" || true

  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=""
}

# Les neuf états de la page : sept en clair, Accueil et Historique en sombre.
all_shots() {
  shot home-light "page=home" light
  shot home-empty-light "page=home-empty" light
  shot history-light "page=history" light
  shot history-empty-light "page=history-empty" light
  shot history-menu-light "page=history-menu" light
  shot vocabulary-light "page=vocabulary" light
  shot vocabulary-sheet-light "page=vocabulary-sheet" light
  shot home-dark "page=home" dark
  shot history-dark "page=history" dark
}

if [ "$#" -eq 0 ]; then
  all_shots
  names="home-light home-empty-light history-light history-empty-light history-menu-light vocabulary-light vocabulary-sheet-light home-dark history-dark"
else
  names="$*"
  for name in $names; do
    case $name in
      home-light) shot home-light "page=home" light ;;
      home-empty-light) shot home-empty-light "page=home-empty" light ;;
      history-light) shot history-light "page=history" light ;;
      history-empty-light) shot history-empty-light "page=history-empty" light ;;
      history-menu-light) shot history-menu-light "page=history-menu" light ;;
      vocabulary-light) shot vocabulary-light "page=vocabulary" light ;;
      vocabulary-sheet-light) shot vocabulary-sheet-light "page=vocabulary-sheet" light ;;
      home-dark) shot home-dark "page=home" dark ;;
      history-dark) shot history-dark "page=history" dark ;;
      *)
        echo "capture inconnue : $name" >&2
        exit 2
        ;;
    esac
  done
fi

status=0
for name in $names; do
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
