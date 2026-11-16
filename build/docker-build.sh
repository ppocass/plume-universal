#!/bin/sh
# Compile un projet Tauri 2 pour Ubuntu 24.04 / Linux Mint 22.
# À lancer SUR LE SERVEUR (zcomputer) : docker-build.sh <project_dir> [args...]
set -u

IMAGE=plume-tauri-build:24.04
CACHE_VOLUME=plume-cargo-cache

if [ "$#" -lt 1 ]; then
  echo "usage: $(basename "$0") <project_dir> [extra args]" >&2
  exit 2
fi

PROJECT_DIR=$1
shift

# docker run -v exige un chemin absolu
case "$PROJECT_DIR" in
  /*) ;;
  *) PROJECT_DIR=$(cd "$PROJECT_DIR" && pwd) || exit 2 ;;
esac

docker run --rm \
  -v "$PROJECT_DIR":/src \
  -v "$CACHE_VOLUME":/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/src/target-linux \
  "$IMAGE" \
  sh -c "cd /src/src-tauri && cargo tauri build --bundles deb $*; rc=\$?; chown -R 1000:1000 /src/target-linux; exit \$rc"
