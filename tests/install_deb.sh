#!/bin/sh
# Installe le .deb Plume dans un conteneur Ubuntu VIERGE et vérifie qu'il
# tourne. À lancer SUR LE SERVEUR (zcomputer), pas dans l'image de build :
#
#   tests/install_deb.sh
#   tests/install_deb.sh /chemin/plume_0.1.0_amd64.deb
#   tests/install_deb.sh /chemin/vers/dossier-de-deb
#   tests/install_deb.sh plume-target-pk        # un volume Docker
#
# Ubuntu 24.04 = la base de Linux Mint 22, la cible. Ubuntu 22.04 est tenté
# ensuite : s'il échoue sur la glibc, c'est la version minimale qui est en
# cause, et le script le dit sans faire échouer le test 24.04.
set -eu

IMAGE_24=ubuntu:24.04
IMAGE_22=ubuntu:22.04
VOLUME_DEFAUT=plume-target-pk
TMP=$(mktemp -d /tmp/plume-deb-test.XXXXXX)
trap 'rm -rf "$TMP"' EXIT

# --- Trouver le .deb ------------------------------------------------------

SOURCE=${1:-}
if [ -z "$SOURCE" ]; then
  if [ -d target-linux/release/bundle/deb ]; then
    SOURCE=target-linux/release/bundle/deb
  else
    SOURCE=$VOLUME_DEFAUT
  fi
fi
echo "Source : $SOURCE"

case "$SOURCE" in
  *.deb)
    cp "$SOURCE" "$TMP"/
    ;;
  *)
    if [ -d "$SOURCE" ]; then
      cp "$SOURCE"/*.deb "$TMP"/
    else
      # Un volume Docker : on sort le .deb avec un conteneur jetable.
      docker run --rm -v "$SOURCE":/t -v "$TMP":/out "$IMAGE_24" \
        sh -c 'cp /t/release/bundle/deb/*.deb /out/'
    fi
    ;;
esac

DEB=$(ls "$TMP"/*.deb 2>/dev/null | head -n 1)
[ -n "$DEB" ] || { echo "Aucun .deb trouvé (source : $SOURCE)" >&2; exit 2; }
DEB_NAME=$(basename "$DEB")
echo "Paquet : $DEB_NAME"
echo

# --- Ubuntu 24.04 (Mint 22) : doit réussir --------------------------------

echo "=== Ubuntu 24.04 : installation et lancement ==="
set +e
docker run --rm -v "$TMP":/deb "$IMAGE_24" sh -c "
  apt-get update -qq &&
  apt-get install -y -qq /deb/$DEB_NAME &&
  plume --version &&
  plume --check;
  if ldd /usr/bin/plume | grep 'not found'; then echo 'BIBLIOTHÈQUE MANQUANTE'; exit 1; fi
  dpkg -L plume
" >"$TMP/24.04.log" 2>&1
STATUS_24=$?
set -e
cat "$TMP/24.04.log"

if [ "$STATUS_24" -ne 0 ] ||
   ! grep -q '^Plume 0.1.0$' "$TMP/24.04.log" ||
   ! grep -q '^session:' "$TMP/24.04.log" ||
   grep -q 'BIBLIOTHÈQUE MANQUANTE' "$TMP/24.04.log"; then
  echo
  echo "ÉCHEC : le .deb ne s'installe pas ou ne démarre pas sur Ubuntu 24.04." >&2
  exit 1
fi
echo "OK : installation et lancement sur Ubuntu 24.04."
echo

# --- Ubuntu 22.04 : informationnel ----------------------------------------

echo "=== Ubuntu 22.04 : essai (informationnel) ==="
set +e
docker run --rm -v "$TMP":/deb "$IMAGE_22" sh -c "
  apt-get update -qq &&
  apt-get install -y -qq /deb/$DEB_NAME &&
  plume --version
" >"$TMP/22.04.log" 2>&1
STATUS_22=$?
set -e
tail -n 12 "$TMP/22.04.log"

if [ "$STATUS_22" -eq 0 ] && ! grep -q 'GLIBC_2.39' "$TMP/22.04.log"; then
  echo "Ubuntu 22.04 : installation et lancement OK."
  SUPPORT_22=ok
else
  echo
  if grep -q 'GLIBC_2.39' "$TMP/22.04.log"; then
    echo "Ubuntu 22.04 : installation possible, mais le chargeur réclame GLIBC_2.39"
    echo "(avertissement « weak version ») : non supporté."
  else
    echo "Ubuntu 22.04 : ÉCHEC (glibc trop ancienne)."
  fi
  echo "Version minimale : Ubuntu 24.04 / Linux Mint 22 (glibc 2.39)."
  SUPPORT_22=non
fi

echo
if [ "$SUPPORT_22" = ok ]; then
  echo "Résultat : Ubuntu 24.04 OK, Ubuntu 22.04 OK."
else
  echo "Résultat : Ubuntu 24.04 OK, Ubuntu 22.04 non supporté (glibc 2.39)."
fi
