#!/bin/sh
# Captures de la pastille (spec §3) : écoute, traitement, ✓ collé et pop-up
# « rien à coller », dans les deux emplacements et les deux thèmes.
#
# À LANCER DANS LE CONTENEUR DE BUILD :
#   build/remote.sh pl 'cd crates/plume-app/src-tauri && cargo build --release -p plume-app >/dev/null && cd /src && PLUME_SHOTS=/src/docs/shots/pl sh tests/shot_pl.sh'
#
# Xvfb 1280×800, session D-Bus si elle manque, binaire release, 4 s par état,
# puis import -window root. La fenêtre est transparente : la racine X (noire
# sous Xvfb) doit se voir autour de la carte — c'est ce qu'on vérifie aussi.
set -eu

# Le bus de session (instance unique) : on relance le script dedans sinon.
if [ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ] && command -v dbus-run-session >/dev/null 2>&1; then
  exec dbus-run-session -- "$0" "$@"
fi

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
BIN=${PLUME_BIN:-$ROOT/target-linux/release/plume}
SHOTS=${PLUME_SHOTS:-$ROOT/docs/shots/pl}
DATA=${PLUME_DATA_DIR:-/tmp/plume-pl}
DISPLAY_NUMBER=99

if [ ! -x "$BIN" ]; then
  echo "binaire absent : $BIN (lancer cargo build --release -p plume-app)" >&2
  exit 1
fi

mkdir -p "$SHOTS" "$DATA"

XVFB_PID=""
if ! xdpyinfo -display ":$DISPLAY_NUMBER" >/dev/null 2>&1; then
  Xvfb ":$DISPLAY_NUMBER" -screen 0 1280x800x24 >/tmp/plume-pl-xvfb.log 2>&1 &
  XVFB_PID=$!
  tries=0
  while ! xdpyinfo -display ":$DISPLAY_NUMBER" >/dev/null 2>&1; do
    tries=$((tries + 1))
    if [ "$tries" -gt 50 ]; then
      echo "Xvfb ne répond pas (voir /tmp/plume-pl-xvfb.log)" >&2
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

# Le look de la pastille (haut ou bas) vient des réglages : on pose le
# settings.json avant de lancer l'app, comme le ferait Réglages → Pastille.
write_settings() {
  style=$1
  theme=$2
  mkdir -p "$DATA"
  cat >"$DATA/settings.json" <<EOF
{"pillStyle":"$style","listenTint":"cyan","thinkTint":"amber","appearance":"$theme","appLanguage":"en","onboardingDone":true}
EOF
}

# shot <nom> <style> <thème> <PLUME_DEMO>
shot() {
  name=$1
  style=$2
  theme=$3
  demo=$4

  write_settings "$style" "$theme"

  PLUME_DATA_DIR="$DATA" PLUME_SHOW=pill PLUME_THEME="$theme" PLUME_DEMO="$demo" \
    "$BIN" >"/tmp/plume-pl-$name.log" 2>&1 &
  APP_PID=$!

  sleep 4
  import -window root -define png:compression-level=3 "$SHOTS/$name.png" \
    2>"/tmp/plume-pl-$name-import.log" || true

  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=""
  sleep 0.5
}

# Les huit états de la commande : quatre phases × deux emplacements, plus le
# thème sombre pour chaque phase (les deux thèmes sont exigés).
shot top-light-listening top light listening
shot bottom-light-listening bottom light listening
shot top-light-processing top light processing
shot bottom-light-processing bottom light processing
shot top-light-pasted top light pasted
shot bottom-light-pasted bottom light pasted
shot top-light-popup top light popup
shot bottom-light-popup bottom light popup
shot top-dark-listening top dark listening
shot bottom-dark-listening bottom dark listening
shot top-dark-processing top dark processing
shot bottom-dark-processing bottom dark processing
shot top-dark-pasted top dark pasted
shot bottom-dark-pasted bottom dark pasted
shot top-dark-popup top dark popup
shot bottom-dark-popup bottom dark popup

status=0
for name in \
  top-light-listening bottom-light-listening top-light-processing bottom-light-processing \
  top-light-pasted bottom-light-pasted top-light-popup bottom-light-popup \
  top-dark-listening bottom-dark-listening top-dark-processing bottom-dark-processing \
  top-dark-pasted bottom-dark-pasted top-dark-popup bottom-dark-popup; do
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
