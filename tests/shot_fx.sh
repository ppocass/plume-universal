#!/bin/sh
# Captures de la passe finale « fx » (docs/a-corriger.md), À LANCER DANS LE
# CONTENEUR DE BUILD :
#   build/remote.sh fx 'cd crates/plume-app/src-tauri && cargo build --release -p plume-app >/dev/null && cd /src && PLUME_SHOTS=/src/docs/shots/fx sh tests/shot_fx.sh'
#
# Même préambule que tests/ui_shot.sh (Xvfb 1280×800, session D-Bus, rendu
# logiciel) ; les états internes sont joués par PLUME_DEMO, que le JS de la page
# lit dans `ui_env`.
#
# Usage : sh tests/shot_fx.sh [nom-de-capture...]
#   sans argument : toutes les captures ; sinon seulement celles nommées.
set -eu

# Le bus de session (instance unique, zone de notification) : on relance le
# script dedans quand il n'y en a pas encore.
if [ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ] && command -v dbus-run-session >/dev/null 2>&1; then
  exec dbus-run-session -- "$0" "$@"
fi

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
BIN=${PLUME_BIN:-$ROOT/target-linux/release/plume}
SHOTS=${PLUME_SHOTS:-$ROOT/docs/shots/fx}
DATA=${PLUME_DATA_DIR:-/tmp/plume-fx}
DISPLAY_NUMBER=99

if [ ! -x "$BIN" ]; then
  echo "binaire absent : $BIN (lancer cargo build --release -p plume-app)" >&2
  exit 1
fi

mkdir -p "$SHOTS" "$DATA"

XVFB_PID=""
if ! xdpyinfo -display ":$DISPLAY_NUMBER" >/dev/null 2>&1; then
  Xvfb ":$DISPLAY_NUMBER" -screen 0 1280x800x24 >/tmp/plume-fx-xvfb.log 2>&1 &
  XVFB_PID=$!
  tries=0
  while ! xdpyinfo -display ":$DISPLAY_NUMBER" >/dev/null 2>&1; do
    tries=$((tries + 1))
    if [ "$tries" -gt 50 ]; then
      echo "Xvfb ne répond pas (voir /tmp/plume-fx-xvfb.log)" >&2
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

# Les réglages de la pastille (style et thème) sont lus au démarrage : on les
# pose avant de lancer l'app, comme le ferait Réglages → Pastille.
write_settings() {
  style=$1
  theme=$2
  language=${3:-en}
  mkdir -p "$DATA"
  cat >"$DATA/settings.json" <<EOF
{"pillStyle":"$style","listenTint":"cyan","thinkTint":"amber","appearance":"$theme","appLanguage":"$language","onboardingDone":true}
EOF
}

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

# La page à ouvrir : un clic sur la ligne de la barre latérale (mêmes repères
# que tests/shot_pb.sh).
open_page() {
  page=$1
  case "$page" in
    home) return 0 ;;
    history) row=2 ;;
    vocabulary) row=3 ;;
    leet) row=4 ;;
    models) row=5 ;;
    settings) row=6 ;;
    *) return 0 ;;
  esac
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

  if [ "$show" = "pill" ]; then
    write_settings "${PILL_STYLE:-top}" "$theme"
  else
    write_settings "top" "$theme" "${PLUME_LANG:-en}"
  fi

  PLUME_DATA_DIR="$DATA" PLUME_SHOW="$show" PLUME_THEME="$theme" PLUME_DEMO="$demo" \
    "$BIN" >"/tmp/plume-fx-$name.log" 2>&1 &
  APP_PID=$!

  if ! wait_window; then
    echo "fenêtre absente pour $name (voir /tmp/plume-fx-$name.log)" >&2
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
    APP_PID=""
    return 1
  fi

  sleep 3
  if [ -n "$page" ] && [ "$page" != "home" ]; then
    open_page "$page"
    sleep 3
  fi

  import -window root -define png:compression-level=3 "$SHOTS/$name.png" \
    2>"/tmp/plume-fx-$name-import.log" || true

  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=""
  WIN=""
  sleep 0.6
}

# Une capture de pastille : le style est passé au lancement (PILL_STYLE).
pill_shot() {
  name=$1
  style=$2
  theme=$3
  demo=$4
  PILL_STYLE="$style" shot "$name" pill "$theme" "$demo"
}

# --- Les captures de la commande ------------------------------------------
# Les touches (Accueil, Historique) et les textes « Mac » (Modèles, 1337),
# l'assistant en téléchargement, puis la pastille dans ses deux styles.

shot home-light main light "page=home"
shot history-light main light "page=history"
shot models-light main light "page=models" models
shot leet-light main light "page=leet" leet
shot onboarding-step6-downloading onboarding light "step=6-downloading"
shot settings-light main light "page=settings" settings
# La variante française : « Alt gauche », « Cet ordinateur ».
PLUME_LANG=fr shot home-light-fr main light "page=home"

pill_shot top-light-listening top light listening
pill_shot bottom-light-listening bottom light listening
pill_shot bottom-dark-listening bottom dark listening
pill_shot top-light-pasted top light pasted
pill_shot top-light-popup top light popup
pill_shot bottom-light-popup bottom light popup
pill_shot bottom-dark-popup bottom dark popup

ALL="home-light history-light models-light leet-light onboarding-step6-downloading settings-light home-light-fr \
top-light-listening bottom-light-listening bottom-dark-listening \
top-light-pasted top-light-popup bottom-light-popup bottom-dark-popup"

if [ "$#" -gt 0 ]; then
  WANTED="$*"
else
  WANTED="$ALL"
fi

status=0
for name in $WANTED; do
  file="$SHOTS/$name.png"
  if [ ! -s "$file" ]; then
    echo "capture manquante : $file" >&2
    status=1
    continue
  fi
  size=$(wc -c <"$file")
  if [ "$size" -lt 2048 ]; then
    echo "capture trop petite : $file ($size octets)" >&2
    status=1
  else
    echo "$file ($size octets)"
  fi
done

exit "$status"
