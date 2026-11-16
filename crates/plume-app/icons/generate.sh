#!/bin/sh
# Fabrique les images de l'app avec ImageMagick (aucun outil graphique à installer) :
#   - icons/source-1024.png : la tuile de l'app (dégradé #7C8CFF → #4338CA, plume blanche)
#   - icons/tray.png        : la plume blanche 22 × 22 de la zone de notification
#   - ui/assets/app-icon.png: la même tuile en 256 px pour l'assistant
# Puis `cargo tauri icon` décline la tuile en toutes les tailles du bundle.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)

# La plume : une silhouette pleine, la tige et trois barbes en transparence.
feather="M 520,150 C 642,232 716,378 690,522 C 666,650 566,760 438,830 C 382,860 326,878 286,884 C 302,792 332,692 380,602 C 444,486 474,326 520,150 Z"
shaft="M 520,150 C 474,330 430,560 300,878"
barb1="M 480,352 C 548,336 612,356 656,404"
barb2="M 424,556 C 500,536 572,560 620,614"
barb3="M 366,730 C 426,716 478,734 510,774"
barb4="M 330,800 C 372,790 404,802 424,824"

# La tuile : dégradé du coin haut-gauche vers le coin bas-droit, découpé en
# carré arrondi (rayon 230 sur 1024, la forme des icônes macOS).
convert -size 1024x1024 -define gradient:angle=135 gradient:'#7C8CFF-#4338CA' \
  \( -size 1024x1024 xc:none -fill white -draw 'roundrectangle 0,0 1023,1023 230,230' \) \
  -alpha set -compose DstIn -composite \
  -fill white -stroke none -draw "path '$feather'" \
  -fill none -stroke 'rgba(67,56,202,0.55)' -strokewidth 20 -draw "path '$shaft'" \
  -fill none -stroke 'rgba(67,56,202,0.38)' -strokewidth 12 -draw "path '$barb1'" \
  -fill none -stroke 'rgba(67,56,202,0.38)' -strokewidth 12 -draw "path '$barb2'" \
  -fill none -stroke 'rgba(67,56,202,0.36)' -strokewidth 11 -draw "path '$barb3'" \
  -fill none -stroke 'rgba(67,56,202,0.32)' -strokewidth 10 -draw "path '$barb4'" \
  "$here/source-1024.png"

# La plume seule, blanche sur fond transparent, 22 × 22.
convert -size 1024x1024 xc:none -fill white -stroke none -draw "path '$feather'" \
  -resize 22x22! -depth 8 -type TrueColorAlpha PNG32:"$here/tray.png"

mkdir -p "$root/ui/assets"
convert "$here/source-1024.png" -resize 256x256! "$root/ui/assets/app-icon.png"

# Les icônes de bureau (icon.png, 32x32.png, 128x128.png, icon.icns, icon.ico…).
(cd "$root/src-tauri" && cargo tauri icon "$here/source-1024.png" -o "$here")
# On ne garde que le bureau (Linux/Windows) : ni Android, ni iOS, ni le Store.
rm -rf "$here/android" "$here/ios"
rm -f "$here"/Square*.png "$here/StoreLogo.png"
# `cargo tauri icon` peut réécrire le dossier : on repose la plume de la zone de notification.
convert -size 1024x1024 xc:none -fill white -stroke none -draw "path '$feather'" \
  -resize 22x22! -depth 8 -type TrueColorAlpha PNG32:"$here/tray.png"

echo "icônes prêtes : $here/source-1024.png, $here/tray.png, $root/ui/assets/app-icon.png"
