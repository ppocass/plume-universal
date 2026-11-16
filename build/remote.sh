#!/bin/sh
# Lancé DEPUIS LE MAC. Copie le projet sur zcomputer et y lance une commande dans l'image de build.
# usage : build/remote.sh <tag> <commande...>
#   <tag>  : nom court du chantier (un dossier par bras : ~/plume-tauri-src/<tag>, cible séparée)
#   ex.    : build/remote.sh core 'cargo test -p plume-core'
# Dans le conteneur : /src = le projet, /models = modèles de test, /sherpa = libs sherpa-onnx prêtes,
#                     /wav = fichiers audio de test.
set -eu
[ "$#" -ge 2 ] || { echo "usage: build/remote.sh <tag> <commande...>" >&2; exit 2; }
TAG=$1; shift
HOST=zcomputer-ts
ROOT=$(cd "$(dirname "$0")/.." && pwd)
DEST=/home/ppocass/plume-tauri-src/$TAG
ssh "$HOST" "mkdir -p $DEST"
rsync -a --delete --exclude target/ --exclude target-linux/ --exclude .git/ --exclude docs/ "$ROOT"/ "$HOST:$DEST/"
CMD=$*
ssh "$HOST" "docker run --rm \
  -v $DEST:/src \
  -v plume-target-$TAG:/src/target-linux \
  -v plume-cargo-cache:/usr/local/cargo/registry \
  -v /home/ppocass/bench-plume/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8:/models/parakeet-v3:ro \
  -v /home/ppocass/bench-plume/sherpa-onnx-v2.0.1-linux-x64-shared:/sherpa:ro \
  -v /home/ppocass/bench-plume/wav:/wav:ro \
  -e CARGO_TARGET_DIR=/src/target-linux \
  -e PLUME_TEST_MODEL=/models/parakeet-v3 \
  -e PLUME_TEST_WAV=/wav \
  -w /src plume-tauri-build:24.04 sh -c '$CMD'"
