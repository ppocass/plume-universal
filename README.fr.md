# Plume

> **Version de test (0.1.1).** Installée sur un Ubuntu 24.04 vierge, puis essayée sur un bureau XFCE complet
> (assistant, toutes les pages, dictée réelle d'une voix enregistrée, texte collé dans un éditeur). La 0.1.1 corrige
> cinq défauts de la 0.1.0 (voir la release). **Pas encore essayée avec un vrai micro sur un vrai PC**, ni sous
> Wayland, ni sous Windows : vos retours sont les bienvenus dans les *issues*.
> Sur Mac, la version native est [Plume](https://github.com/ppocass/plume). Cette version-ci remplace la bêta
> [plume-linux](https://github.com/ppocass/plume-linux).

Plume est une application de dictée locale pour Linux et Windows. On parle, le
texte s'écrit dans la fenêtre qui a le focus, et l'audio ne quitte jamais
l'ordinateur : la reconnaissance vocale tourne sur le processeur, hors ligne.

Le moteur est **NVIDIA Parakeet TDT 0.6B v3** (int8, 25 langues européennes) via
[sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) 2.0.1. Whisper small et base
sont aussi proposés dans l'application pour les autres langues.

Rien ne sort de l'ordinateur, à une exception que vous contrôlez : le « lissage »
facultatif (ponctuation, accents, orthographe) envoie le **texte** transcrit, pas
l'audio, au service compatible OpenAI réglé dans les Réglages. Laissez-le
désactivé et Plume est entièrement hors ligne.

## Installation (Linux Mint 22 / Ubuntu 24.04)

Le paquet s'appelle `plume_0.1.1_amd64.deb` (environ 18 Mo à télécharger, 56 Mo
installé, plus le modèle de dictée téléchargé au premier lancement, environ
640 Mo).

1. Téléchargez `plume_0.1.1_amd64.deb` depuis la page des releases.
2. **Double-cliquez sur le fichier.** Le Gestionnaire de logiciels (ou GDebi)
   s'ouvre : cliquez sur *Installer*, tapez votre mot de passe, c'est fini.
   L'icône Plume apparaît ensuite dans le menu des applications.
3. Ou, dans un terminal :

   ```sh
   sudo apt install ./plume_0.1.1_amd64.deb
   ```

Lien direct : [plume_0.1.1_amd64.deb](https://github.com/ppocass/plume-universal/releases/latest/download/plume_0.1.1_amd64.deb).

Ubuntu 24.04 et Linux Mint 22 sont le minimum : le binaire a besoin de la glibc
2.39. Ubuntu 22.04 (glibc 2.35) peut installer le paquet mais le chargeur
avertit à propos de `GLIBC_2.39` ; ce n'est **pas pris en charge**.

Le workflow de release construit aussi le .deb sur Ubuntu 22.04
(`plume_0.1.1_amd64_ubuntu-22.04.deb`) pour les systèmes plus anciens. Cette
construction n'a pas été testée ici.

### Premier lancement

Le premier démarrage ouvre l'assistant de mise en route, en huit étapes :
bienvenue, micro, insertion du texte, raccourci, langue, modèle, lissage,
terminé. Il propose :

- **Micro** : le périphérique d'entrée par défaut, avec un test de niveau.
- **Insertion du texte** : sous X11, Plume colle avec `xdotool` (installé comme
  dépendance du .deb).
- **Raccourci** : **Alt gauche** par défaut, modifiable (Alt droit, Ctrl droit,
  Super droit, F1-F24).
- **Langue** : automatique, ou une des 25 langues de Parakeet.
- **Modèle** : Parakeet TDT 0.6B v3, téléchargé au premier usage (environ
  640 Mo). Whisper small et base sont proposés pour les autres langues.
- **Lissage** : facultatif, désactivé par défaut ; demande une adresse et une
  clé.
- **Terminé**.

Ensuite, Plume vit dans la zone de notification. Fermer la fenêtre ne quitte
pas l'application : c'est *Quitter Plume* dans le menu de la zone de
notification.

Plume démarre à l'ouverture de session par défaut (une entrée d'autostart est
écrite dans `~/.config/autostart/plume.desktop`). L'interrupteur est dans les
Réglages.

## Les gestes

- **Appuyez sur Alt gauche** pour commencer à dicter ; appuyez à nouveau pour
  arrêter. Le texte est alors collé dans la fenêtre active. Un appui, c'est une
  pression puis un relâchement de la touche seule, en moins de 0,8 s :
  Alt+lettre et Alt+clic continuent de fonctionner normalement.
- **Double appui sur Alt gauche** pendant l'enregistrement pour annuler : rien
  n'est transcrit, rien n'est collé.
- **Maintenez Super au moment d'arrêter** pour obtenir la dictée en 1337 (leet
  speak). La touche se règle dans la page 1337.
- La pastille en haut ou en bas de l'écran montre l'état : écoute, réflexion,
  collé, annulé.

L'historique garde chaque dictée (texte et date), avec recherche, copie et
suppression. Le menu de la zone de notification contient *Start Dictation*,
*Copy Last Dictation*, *History*, *Settings*, *Support Plume* et *Quit Plume*.

## X11 et Wayland

Plume insère le texte en collant, ce qui demande un raccourci global et un moyen
d'envoyer Ctrl+V à la fenêtre active :

- **X11** (la session par défaut de Linux Mint 22) : ça marche. `xdotool` est
  installé avec le .deb.
- **Wayland** : non pris en charge. La lecture du raccourci global et le collage
  dans une autre application sont limités par le compositeur ; l'application
  cherche `ydotool` mais il n'est pas empaqueté. Utilisez une session X11.

## Vitesse

Mesuré sur un Intel Core i7-6700 (4 cœurs / 8 threads, sans carte graphique),
avec sherpa-onnx 2.0.1 et le modèle Parakeet int8, 4 threads :

| Audio | Inférence | Facteur temps réel |
|---|---:|---:|
| 30,6 s de parole française | **2,3 s** | 0,074 |

Le chargement du modèle prend environ 2,5 s, une fois par lancement. Parakeet ne
dépasse jamais un RTF de 0,175 sur les fichiers de test, alors que Whisper
large-v3-turbo met 45 s sur les mêmes 30,6 s d'audio. Chiffres complets et
journaux bruts : `docs/bench-zcomputer.md`.

Le silence, le bruit de pièce et un clic de souris donnent une chaîne vide
(aucune phrase inventée), c'est pourquoi Plume ne colle rien dans ces cas.

## Ce qui n'est pas encore testé

- Un **vrai micro sur une machine physique**. Tout a été testé en conteneur
  (modèle, moteur, empaquetage, X11 sous Xvfb), mais pas avec une personne qui
  parle dans un vrai micro sur un vrai bureau.
- **Wayland**.
- **Windows**. Le code, le workflow et la configuration de l'installeur sont là,
  mais aucune construction Windows n'a jamais été exécutée. Considérez la
  première construction Windows comme expérimentale.

## Désinstallation

```sh
sudo apt remove plume
```

Les réglages, l'historique et les modèles téléchargés restent :

```sh
rm -rf ~/.config/plume ~/.local/share/plume
rm -f ~/.config/autostart/plume.desktop
```

## Licence

Plume est sous MIT. Composants tiers :

- **sherpa-onnx** 2.0.1, Apache-2.0 (<https://github.com/k2-fsa/sherpa-onnx>),
  avec onnxruntime (MIT).
- **Parakeet TDT 0.6B v3**, NVIDIA, **CC-BY-4.0** — la fiche du modèle est
  <https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3> (« License: cc-by-4.0 »).
  Le modèle est téléchargé par l'application, pas livré dans le paquet.
- **Inter** (police de l'interface), SIL Open Font License 1.1.
- **Lucide** (icônes), ISC.

Les textes complets des licences de la police et des icônes sont dans
`crates/plume-app/ui/fonts/LICENSE.txt` et `crates/plume-app/ui/icons/LICENSE`.
