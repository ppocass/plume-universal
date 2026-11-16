#!/bin/sh
# Bout en bout de la dictée sous X11 — « Plume sans interface ».
#
# À lancer DANS LE CONTENEUR DE BUILD, à la racine du projet (c'est ce que fait
# l'acceptation) :
#   cargo build -p plume-cli --release && sh tests/e2e_x11.sh
#
# Le scénario est celui de la vraie dictée : on appuie sur ⌥ (début), on parle
# (ici `PLUME_FAKE_AUDIO` remplace le micro par court.wav), on appuie encore
# (fin), et le texte doit arriver au curseur. Aucune fenêtre ne reçoit le
# Ctrl+V dans ce conteneur : le texte est lu dans `PLUME_PASTE_LOG`, le journal
# que le Paster réel tient en plus du collage (voir controller/real.rs).
set -eu

BIN=${PLUME_BIN:-target-linux/release/plume-cli}
MODEL=${PLUME_TEST_MODEL:-/models/parakeet-v3}
WAV_DIR=${PLUME_TEST_WAV:-/wav}
PASTE_LOG=${PLUME_PASTE_LOG:-/tmp/paste.log}
STDERR_LOG=/tmp/plume-e2e-stderr.log
XVFB_LOG=/tmp/plume-xvfb.log
DISPLAY_NUM=:99

[ -x "$BIN" ] || {
    echo "binaire absent : $BIN (cargo build -p plume-cli --release)" >&2
    exit 1
}

rm -f "$PASTE_LOG" "$STDERR_LOG"

# Xvfb avec RECORD : rdev lit les touches par XRecord (aucun grab, aucun droit
# root). XTEST, dont xdotool a besoin pour simuler les appuis, est fourni par
# défaut par Xvfb.
Xvfb "$DISPLAY_NUM" +extension RECORD -screen 0 1280x800x24 >"$XVFB_LOG" 2>&1 &
XVFB_PID=$!
CLI_PID=

cleanup() {
    if [ -n "$CLI_PID" ]; then
        kill "$CLI_PID" 2>/dev/null || true
    fi
    kill "$XVFB_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

export DISPLAY=$DISPLAY_NUM

# Attendre que le serveur X réponde.
tries=0
while ! xdpyinfo >/dev/null 2>&1; do
    tries=$((tries + 1))
    if [ "$tries" -gt 100 ]; then
        echo "Xvfb ne répond pas sur $DISPLAY_NUM" >&2
        cat "$XVFB_LOG" >&2 || true
        exit 1
    fi
    sleep 0.1
done

PLUME_FAKE_AUDIO="$WAV_DIR/court.wav" PLUME_PASTE_LOG="$PASTE_LOG" \
    "$BIN" run --model "$MODEL" >"$STDERR_LOG" 2>&1 &
CLI_PID=$!

# L'écoute clavier s'installe en quelques centaines de ms ; on laisse de la
# marge. Premier appui : début de la dictée (et chargement du modèle en fond).
sleep 2
xdotool keydown Alt_L keyup Alt_L

# Vérifier que le geste est bien arrivé avant de compter les 3 s d'audio : un
# échec ici se voit tout de suite, avec le journal, au lieu de tourner 20 s.
tries=0
while [ "$tries" -lt 50 ]; do
    if grep -q "enregistrement" "$STDERR_LOG" 2>/dev/null; then
        break
    fi
    if ! kill -0 "$CLI_PID" 2>/dev/null; then
        break
    fi
    sleep 0.1
    tries=$((tries + 1))
done
if ! grep -q "enregistrement" "$STDERR_LOG" 2>/dev/null; then
    echo "ÉCHEC : la dictée n'a pas démarré (le geste n'est pas arrivé)" >&2
    cat "$STDERR_LOG" >&2 || true
    exit 1
fi

# L'audio factice « se joue » : on laisse la dictée durer, puis second appui.
sleep 3
xdotool keydown Alt_L keyup Alt_L

# Le Stop attend la fenêtre du double appui (0,3 s), puis charge le modèle
# (~2,5 s) et transcrit court.wav (~0,5 s). 20 s de marge.
tries=0
while [ "$tries" -lt 200 ]; do
    if [ -f "$PASTE_LOG" ] && grep -q "Salut" "$PASTE_LOG"; then
        echo "OK : $(cat "$PASTE_LOG")"
        exit 0
    fi
    # Le processus est mort ? Inutile d'attendre les 20 s.
    if ! kill -0 "$CLI_PID" 2>/dev/null; then
        break
    fi
    sleep 0.1
    tries=$((tries + 1))
done

echo "ÉCHEC : « Salut » n'est jamais arrivé dans $PASTE_LOG" >&2
echo "--- stderr de plume-cli ---" >&2
cat "$STDERR_LOG" >&2 || true
if [ -f "$PASTE_LOG" ]; then
    echo "--- journal de collage ---" >&2
    cat "$PASTE_LOG" >&2
fi
exit 1
