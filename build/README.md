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

## L'application Plume (crate `plume-app`, Tauri 2)

L'app vit dans `crates/plume-app` : le code Rust et `tauri.conf.json` dans
`crates/plume-app/src-tauri/` (disposition Tauri standard), le front en
HTML/CSS/JS natif dans `crates/plume-app/ui/` (aucun npm, aucun bundler).
`build/remote.sh` synchronise le projet, puis la commande tourne dans l'image
`plume-tauri-build:24.04` (cwd `/src`).

### Fabriquer le `.deb`

```sh
# 1. Les images de l'app (tuile dégradée + plume) et les icônes du bundle.
build/remote.sh app 'cd crates/plume-app && sh icons/generate.sh'

# 2. Le paquet Debian (binaire release + ressources + libs sherpa-onnx).
#    Les .so (libsherpa-onnx + libonnxruntime) sont copiés dans vendor/sherpa/
#    avant la compilation : le bundle les installe dans /usr/bin, à côté du
#    binaire, et le rpath $ORIGIN les fait trouver sans LD_LIBRARY_PATH.
build/remote.sh app 'mkdir -p crates/plume-app/src-tauri/vendor/sherpa && cp -a /sherpa/lib/*.so* crates/plume-app/src-tauri/vendor/sherpa/ && cd crates/plume-app && cargo tauri build --bundles deb'
```

Le `.deb` atterrit dans `target-linux/release/bundle/deb/` dans le conteneur
(`CARGO_TARGET_DIR=/src/target-linux`) : c'est le volume Docker
`plume-target-app`, pas un dossier du projet sur l'hôte. Vérifier son contenu :

```sh
ssh zcomputer-ts 'docker run --rm -v plume-target-app:/t alpine ls /t/release/bundle/deb/'
ssh zcomputer-ts 'docker run --rm -v plume-target-app:/t plume-tauri-build:24.04 \
  dpkg-deb -c /t/release/bundle/deb/*.deb | grep -E "usr/bin/(plume|libsherpa|libonnxruntime)"'
```

### Les captures de la coquille (Xvfb)

`tests/ui_shot.sh` tourne **dans le conteneur** : Xvfb `:99` en 1280×800,
session D-Bus (`dbus-run-session`), binaire release, 6 s par capture, puis
`import -window root`. Trois images : `main` (clair), `onboarding`, `main-dark`
(`PLUME_THEME=dark`). Le script sort en 1 si une image manque ou fait moins de
20 Ko.

```sh
build/remote.sh app 'sh tests/ui_shot.sh'
mkdir -p docs/shots
scp zcomputer-ts:/home/ppocass/plume-tauri-src/app/docs/shots/*.png docs/shots/
```

Les variables utiles : `PLUME_BIN` (chemin du binaire), `PLUME_SHOTS` (dossier de
sortie), `PLUME_SHOW` (fenêtre montrée au démarrage : `main`, `onboarding`,
`pill`) et `PLUME_THEME` (`dark` ou `light`), lues par le binaire.

## Installation d'un utilisateur : le paquet `.deb`

> Les deux sections précédentes datent d'avant le paquet installable : les
> `.so` vont désormais dans `/usr/lib/plume` (plus dans `/usr/bin`) et les
> commandes ci-dessous utilisent le tag `pk` (volume `plume-target-pk`).

Le paquet est fait pour Linux Mint 22 / Ubuntu 24.04 (« install » en un
double-clic, ou `sudo apt install ./plume_0.1.0_amd64.deb`). Il s'appelle
`plume_0.1.0_amd64.deb` : `Package: plume`, `Section: utils`,
`Maintainer: Pierre Pocquet-Assemien`, `Homepage:
https://github.com/ppocass/plume`, description FR puis EN.

### Ce qu'il installe

- `/usr/bin/plume` — le binaire, seul.
- `/usr/lib/plume/lib*.so` — `libsherpa-onnx-c-api.so`,
  `libsherpa-onnx-cxx-api.so`, `libonnxruntime.so`. Ils ne sont **plus** dans
  `/usr/bin` : le binaire porte un `RUNPATH` `$ORIGIN:/usr/lib/plume`
  (`build.rs` de `plume-app` ajoute `/usr/lib/plume` au `$ORIGIN` global de
  `.cargo/config.toml`), donc rien à mettre dans `LD_LIBRARY_PATH`.
- `/usr/share/applications/plume.desktop` — `Name=Plume`,
  `Categories=Utility;Accessibility;`, `StartupWMClass=plume`, `Icon=plume`.
  Le modèle est `crates/plume-app/packaging/plume.desktop`.
- `/usr/share/icons/hicolor/*/apps/plume.png` — les icônes du bundle.
- `Depends: libayatana-appindicator3-1, libwebkit2gtk-4.1-0, libgtk-3-0,
  xdotool` (les trois premiers sont ajoutés par le CLI Tauri d'après
  `--features tray-icon` et la config ; `xdotool` vient de `tauri.conf.json`).

### Le fabriquer

```sh
# Les .so de sherpa-onnx doivent être présents dans vendor/sherpa avant le
# bundle (le dossier est vide dans le dépôt).
build/remote.sh pk 'mkdir -p crates/plume-app/src-tauri/vendor/sherpa && \
  cp -a /sherpa/lib/*.so* crates/plume-app/src-tauri/vendor/sherpa/ && \
  cd crates/plume-app && cargo tauri build --bundles deb'
```

Le `.deb` sort dans `/src/target-linux/release/bundle/deb/` (volume Docker
`plume-target-pk`). Le profil `release` est celui de `cargo tauri build`.

### Le tester sur une installation neuve

`tests/install_deb.sh` tourne **sur le serveur**, pas dans l'image de build : il
lance `ubuntu:24.04` et `ubuntu:22.04` nus, installe le `.deb` avec `apt`, puis
vérifie `plume --version`, `plume --check`, `ldd /usr/bin/plume` (aucune
bibliothèque manquante) et `dpkg -L plume`.

```sh
ssh zcomputer-ts 'cd /home/ppocass/plume-tauri-src/pk && sh tests/install_deb.sh plume-target-pk'
```

Résultat attendu : **Ubuntu 24.04 OK** ; Ubuntu 22.04 s'installe mais le
chargeur réclame `GLIBC_2.39` (« weak version »), donc **non supporté**. Le
minimum est Linux Mint 22 / Ubuntu 24.04, comme le disent les deux README.

### `plume --version` et `plume --check`

Deux options de diagnostic, traitées avant l'ouverture de Tauri (donc sans
écran) :

```sh
plume --version     # Plume 0.1.0
plume --check       # session, xdotool, micros, modèle, dossiers — sortie 0
```

### Démarrage automatique

`tauri-plugin-autostart` écrit `~/.config/autostart/plume.desktop` quand
`launch_at_login` est vrai (défaut). Vérifié sous Xvfb avec un `HOME` neuf :
le fichier apparaît, avec `Exec=/usr/bin/plume` une fois le `.deb` installé.

### AppImage : abandonné

`cargo tauri build --bundles appimage` **fonctionne** (≈ 100 Mo), mais
linuxdeploy n'embarque volontairement pas les bibliothèques de base du bureau
(`libX11`, `libEGL`, `libgbm`, `libfontconfig`, `libasound`…). Dans un
`ubuntu:24.04` nu, l'AppImage ne démarre donc pas :

```
plume: error while loading shared libraries: libX11.so.6: cannot open shared object file
```

et il faut installer une pile graphique complète pour que `--version` réponde.
Embarquer `libEGL`/`libgbm` à la main casserait les pilotes de la machine
hôte. L'AppImage est donc **retiré** (pas de `linux.appimage` dans
`tauri.conf.json`, pas de cible appimage dans le workflow) : le `.deb` est le
seul format livré.

### Windows : préparé, jamais exécuté

`.github/workflows/release.yml` construit aussi un MSI et un NSIS
(`windows-latest`), avec les DLL de
`sherpa-onnx-v2.0.1-win-x64-shared-MT-Release-lib.tar.bz2`. Aucune exécution
Windows n'a eu lieu : c'est une préparation, pas une preuve. À dire tel quel
dans les README.

## Attention au disque de zcomputer
Chaque `build/remote.sh <tag>` crée un volume Docker `plume-target-<tag>` (2 à 8 Go). Le 16/11/2026, 17 volumes ont
rempli le disque à 99 %. Après usage : `ssh zcomputer-ts 'docker volume rm plume-target-<tag>'`. Garder un seul tag
pour le travail courant.
