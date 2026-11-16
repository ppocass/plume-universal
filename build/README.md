# Image Docker de compilation Tauri 2 (Ubuntu 24.04 / Linux Mint 22)

Image `plume-tauri-build:24.04`, construite sur le serveur `zcomputer`, pour
compiler l'app Tauri 2 de Plume en `.deb` dans un environnement Ubuntu 24.04
(compatible Linux Mint 22). Pas de npm/node : le front est déjà construit.

## Contenu de l'image

- Base `ubuntu:24.04`.
- Chaîne de compilation : `build-essential`, `cmake`, `clang`, `libclang-dev`,
  `pkg-config`, `git`, `curl`, `wget`, `file`, `patchelf`, `libssl-dev`.
- Dépendances Tauri/GTK : `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`,
  `libayatana-appindicator3-dev`, `librsvg2-dev`, `libxdo-dev`, `xdg-utils`,
  audio (`libasound2-dev`, `libpulse-dev`), X11 (`libx11-dev`, `libxi-dev`,
  `libxtst-dev`, `libxrandr-dev`).
- Empaquetage : `dpkg-dev`, `fakeroot`, `rpm`, `squashfs-tools`.
- Test/affichage : `xvfb`, `x11-utils`, `xdotool`, `imagemagick`.
- Rust stable (rustup, profil `minimal`) dans `/usr/local/rustup` et
  `/usr/local/cargo`, plus `cargo-tauri` (tauri-cli `^2`).
- `WORKDIR /src`.

## Reconstruire l'image

Depuis le Mac :

```sh
rsync -av build/ zcomputer-ts:/home/ppocass/plume-tauri-build/
ssh zcomputer-ts 'cd /home/ppocass/plume-tauri-build && docker build -t plume-tauri-build:24.04 .'
```

Compter 10 à 20 min (surtout la compilation de `tauri-cli`).

## Utiliser docker-build.sh

Le script tourne **sur le serveur** et compile un projet dans `/src` :

```sh
ssh zcomputer-ts '/home/ppocass/plume-tauri-build/docker-build.sh /home/ppocass/plume-tauri-build/hello'
```

- `<project_dir>` : racine du projet (doit contenir `src-tauri/`).
- Les arguments suivants sont passés à `cargo tauri build` (ex. `--verbose`).
- Le cache Cargo est partagé via le volume nommé `plume-cargo-cache` monté sur
  `/usr/local/cargo/registry`.
- Sortie : `<project_dir>/target-linux/release/bundle/deb/*.deb`
  (`CARGO_TARGET_DIR=/src/target-linux`), appartient ensuite à l'utilisateur
  1000:1000.

## Mesures (16/11/2026, zcomputer)

- Construction de l'image : ≈ 6 min ; Rust 1.100.0, tauri-cli 2.12.5.
- Image `plume-tauri-build:24.04` : **3,84 Go** (couches 3,73 Go, contenu
  compressé 885 Mo).
- Volume `plume-cargo-cache` : 460 Mo après un premier build.
- Build du projet témoin `hello/` (premier build, cache Cargo vide) :
  **2 min 47 s**, dont 2 min 38 s de compilation.
- `.deb` produit : `hello-plume_0.0.1_amd64.deb`, **2 128 322 octets
  (2,1 Mo)**, 5,5 Mo installé (`Depends: libwebkit2gtk-4.1-0, libgtk-3-0`).
- glibc maximale exigée par le binaire : **GLIBC_2.39** — donc Ubuntu 24.04 /
  Linux Mint 22 (Ubuntu 22.04, en 2.35, ne conviendrait pas).

## Espace disque

Prévoir ≈ 3,9 Go pour l'image (une seule fois) et ≈ 460 Mo pour le volume de
cache Cargo, qui grossit avec les dépendances des projets compilés. Le dossier
`target-linux/` d'un projet reste sur l'hôte : ≈ 1,1 Go pour un projet Tauri 2
simple après un build release.
