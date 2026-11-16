#!/bin/sh
# Captures des pages 1337, Modèles, Réglages et de « Nouveautés ».
#
# À LANCER DANS LE CONTENEUR DE BUILD :
#   build/remote.sh pb 'cd crates/plume-app/src-tauri && cargo build --release -p plume-app >/dev/null && cd /src && PLUME_SHOTS=/src/docs/shots/pb sh tests/shot_pb.sh'
#
# Xvfb 1280×800, session D-Bus si elle manque, binaire release, 6 s par état,
# puis import -window root. La page est choisie par le clavier (Tab + Entrée
# dans la barre latérale, aucun anneau de focus), les états internes par
# PLUME_DEMO (que le JS de la page lit dans ui_env).
set -eu

if [ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ] && command -v dbus-run-session >/dev/null 2>&1; then
  exec dbus-run-session -- "$0" "$@"
fi

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
BIN=${PLUME_BIN:-$ROOT/target-linux/release/plume}
SHOTS=${PLUME_SHOTS:-$ROOT/docs/shots/pb}
DATA=${PLUME_DATA_DIR:-/tmp/plume-pb}
DISPLAY_NUMBER=99

if [ ! -x "$BIN" ]; then
  echo "binaire absent : $BIN (lancer cargo build --release -p plume-app)" >&2
  exit 1
fi

mkdir -p "$SHOTS" "$DATA"

XVFB_PID=""
if ! xdpyinfo -display ":$DISPLAY_NUMBER" >/dev/null 2>&1; then
  Xvfb ":$DISPLAY_NUMBER" -screen 0 1280x800x24 >/tmp/plume-pb-xvfb.log 2>&1 &
  XVFB_PID=$!
  tries=0
  while ! xdpyinfo -display ":$DISPLAY_NUMBER" >/dev/null 2>&1; do
    tries=$((tries + 1))
    if [ "$tries" -gt 50 ]; then
      echo "Xvfb ne répond pas (voir /tmp/plume-pb-xvfb.log)" >&2
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
WIN=""
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

# La fenêtre visible que l'app vient d'ouvrir (les autres sont cachées).
wait_window() {
  tries=0
  while [ "$tries" -lt 75 ]; do
    WIN=$(xdotool search --onlyvisible --name '^Plume$' 2>/dev/null | head -n 1 || true)
    if [ -n "$WIN" ]; then
      return 0
    fi
    tries=$((tries + 1))
    sleep 0.2
  done
  return 1
}

# La page à ouvrir : Tab dans la barre latérale (Home = 1), puis Entrée.
open_page() {
  page=$1
  case "$page" in
    history) row=2 ;;
    vocabulary) row=3 ;;
    leet) row=4 ;;
    models) row=5 ;;
    settings) row=6 ;;
    *) return 0 ;;
  esac
  # Un clic sur la ligne de la barre latérale : 24 pt sous le bord haut de la
  # fenêtre pour la première, puis 27 pt par ligne (mesuré sur les captures).
  eval "$(xdotool getwindowgeometry --shell "$WIN")"
  click_x=$((X + 100))
  click_y=$((Y + 24 + 27 * (row - 1)))
  xdotool mousemove --sync "$click_x" "$click_y" click 1
  sleep 0.4
}

# Une capture : nom, fenêtre (PLUME_SHOW), thème, état de démo, page.
shot() {
  name=$1
  show=$2
  theme=${3:-light}
  demo=${4:-}
  page=${5:-}

  PLUME_DATA_DIR="$DATA" PLUME_SHOW="$show" PLUME_THEME="$theme" PLUME_DEMO="$demo" \
    "$BIN" >"/tmp/plume-pb-$name.log" 2>&1 &
  APP_PID=$!

  if ! wait_window; then
    echo "fenêtre absente pour $name (voir /tmp/plume-pb-$name.log)" >&2
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
    APP_PID=""
    return 1
  fi

  sleep 3
  if [ -n "$page" ]; then
    open_page "$page"
    sleep 3
  fi

  import -window root -define png:compression-level=3 "$SHOTS/$name.png" \
    2>"/tmp/plume-pb-$name-import.log" || true

  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=""
  WIN=""
  sleep 0.6
}

shot leet main light page=leet leet
shot leet-converted main light page=leet-converted leet
shot models main light page=models models
shot models-downloading main light page=models-downloading models
shot settings main light page=settings settings
shot settings-cleanup main light page=settings-cleanup settings
shot whatsnew whatsnew light whatsnew
shot models-dark main dark page=models models
shot settings-dark main dark page=settings settings
shot whatsnew-dark whatsnew dark whatsnew

status=0
for name in leet leet-converted models models-downloading settings settings-cleanup whatsnew models-dark settings-dark whatsnew-dark; do
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
