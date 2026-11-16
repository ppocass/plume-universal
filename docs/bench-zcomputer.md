# Benchmark STT sur *zcomputer* — Parakeet TDT 0.6B v3 (int8) vs Whisper large-v3-turbo (q5_0)

Mesures faites le **16/11/2026** sur le serveur `zcomputer` (accès Tailscale), **CPU seul, sans GPU**.
Chaque chiffre de ce document est repris des journaux bruts déposés dans `docs/bench-raw/`
(un fichier par run, stdout et stderr séparés).

---

## 1. Machine

```
$ lscpu | grep -E "Model name|^Architecture|^CPU\(s\)|Thread|Core|Socket|CPU max"
Architecture:                            x86_64
CPU(s):                                  8
Model name:                              Intel(R) Core(TM) i7-6700 CPU @ 3.40GHz
Thread(s) per core:                      2
Core(s) per socket:                      4
Socket(s):                               1
CPU max MHz:                             4000.0000

$ nproc
8

$ free -h
               total        used        free      shared  buff/cache   available
Mem:            31Gi        13Gi       2.0Gi       561Mi        16Gi        17Gi
Swap:           11Gi       8.1Gi       3.4Gi
```

- Debian **13.6**, noyau `6.12.105+deb13-amd64`.
- Jeu d'instructions : **AVX2** et **FMA** présents, **AVX-512 absent**.
- 4 cœurs physiques / 8 threads logiques : `--num-threads=8` dépasse donc le nombre de cœurs physiques.
- La machine n'était **pas au repos** pendant les mesures. Relevé des processus juste après les runs :
  `qbittorrent-nox` (~5,8 Gio de RSS), deux JVM Minecraft (~7,3 Gio au total), et le `whisper-server`
  de production de Plume sur le port **8091** (~1,7 Gio, 4 threads, au repos). Le swap était déjà occupé
  à 8,1 Gio. Aucun de ces services n'a été arrêté, sollicité, ni déplacé ; le port 8090 n'a jamais été utilisé.
  Cette charge explique la variance des runs à 8 threads (voir §5.2).

## 2. Outils et modèles

| | Parakeet | Whisper |
|---|---|---|
| Moteur | **sherpa-onnx v2.0.1** (`sherpa-onnx-v2.0.1-linux-x64-shared`), binaire `bin/sherpa-onnx-offline` | **whisper.cpp** compilé depuis les sources, commit `aba02de` (2026-11-16) |
| Compilation | binaire précompilé de la release (aucune compilation locale) | `cmake -B build -DCMAKE_BUILD_TYPE=Release` puis `cmake --build build -j8 --target whisper-cli` ; gcc 14.2.0, cmake 3.31.6 |
| Modèle | `sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8` : `encoder.int8.onnx` 652 Mo, `decoder.int8.onnx` 11,8 Mo, `joiner.int8.onnx` 6,4 Mo, `tokens.txt` | `ggml-large-v3-turbo-q5_0.bin` (574 Mo), déjà présent dans `/home/ppocass/models/` |
| Type | transducteur NeMo (`--model-type=nemo_transducer`), quantification **int8** | Transformer encodeur-décodeur, quantification **q5_0** |
| Décodage | `greedy_search` (défaut du binaire) | `5 beams + best of 5` (défaut de `whisper-cli`) |
| Langue | multilingue, aucune option de langue exposée par le binaire offline | `-l fr` |

Commandes exactes :

```bash
# Parakeet
LD_LIBRARY_PATH=.../lib  bin/sherpa-onnx-offline \
  --encoder=.../encoder.int8.onnx --decoder=.../decoder.int8.onnx \
  --joiner=.../joiner.int8.onnx --tokens=.../tokens.txt \
  --model-type=nemo_transducer --num-threads=N  wav/<fichier>.wav

# Whisper
whisper.cpp/build/bin/whisper-cli -m /home/ppocass/models/ggml-large-v3-turbo-q5_0.bin \
  -l fr -t N -f wav/<fichier>.wav -nt
```

## 3. Protocole

- 8 WAV de test, tous en **16 kHz mono PCM 16 bits** (`pcm_s16le`), durées mesurées avec `ffprobe` :
  `court.wav` 7,97 s · `long.wav` 30,58 s · `euh.wav` 9,14 s · `noms.wav` 5,20 s ·
  `noms2.wav` 7,47 s · `silence.wav` 4,00 s · `bruit-piece.wav` 5,00 s · `clic.wav` 2,05 s.
  Les cinq premiers sont de la parole TTS française ; les trois derniers sont du non-parole
  (silence, bruit de pièce, clic).
- Un **processus par fichier** (comme le fait l'app : un run = un lancement du binaire), avec `N = 4` et `N = 8`.
- `long.wav` répété **3 fois** par valeur de N ; les autres fichiers une fois. Soit 20 runs par moteur.
- Un run de chauffe (`court.wav`, 4 threads) est exécuté puis jeté avant chaque moteur, pour que le modèle
  soit en cache page et que le premier run ne soit pas pénalisé. Ces runs de chauffe ne sont pas comptés.
- Mesures prises par un harnais Python (`os.fork` + `os.wait4`) : temps **wall** du processus complet et
  `ru_maxrss`, qui est le **pic RSS** du processus — la même grandeur que le « Maximum resident set size »
  de `/usr/bin/time -v`. stdout et stderr sont capturés séparément pour ne pas mélanger le JSON de
  transcription avec les lignes de timing.
- **RTF = temps d'inférence ÷ durée audio**. Le temps de chargement du modèle est donné à part et exclu du
  RTF pour Parakeet, dont le binaire sépare explicitement les deux. Pour Whisper, `whisper-cli` n'imprime
  qu'un `total time` qui **inclut** le chargement (0,26 à 0,47 s) : son RTF est donc marginalement
  bout en bout, ce qui ne change rien à l'ordre de grandeur. La colonne `Wall` donne dans tous les cas le
  temps complet du processus, chargement compris.

## 4. Parakeet TDT 0.6B v3 int8

`sherpa-onnx-offline` imprime lui-même `recognizer created in X s` (chargement) et `Elapsed seconds`
(inférence). Les deux sont repris tels quels.

| Fichier | Durée audio (s) | Threads | Chargement (s) | Inférence (s) | Wall (s) | RTF | RSS max (MB) |
|---|---:|---:|---:|---:|---:|---:|---:|
| court.wav | 7,97 | 4 | 2,52 | 0,55 | 3,15 | 0,069 | 861 |
| long.wav | 30,58 | 4 | 2,51 | 2,27 | 4,88 | 0,074 | 1118 |
| euh.wav | 9,14 | 4 | 2,53 | 0,65 | 3,26 | 0,071 | 872 |
| noms.wav | 5,20 | 4 | 2,54 | 0,39 | 3,02 | 0,076 | 835 |
| noms2.wav | 7,47 | 4 | 2,51 | 0,52 | 3,11 | 0,069 | 856 |
| silence.wav | 4,00 | 4 | 2,56 | 0,32 | 2,96 | 0,080 | 824 |
| bruit-piece.wav | 5,00 | 4 | 2,54 | 0,35 | 2,98 | 0,071 | 833 |
| clic.wav | 2,05 | 4 | 2,57 | 0,18 | 2,82 | 0,087 | 806 |
| court.wav | 7,97 | 8 | 2,55 | 0,67 | 3,32 | 0,084 | 865 |
| long.wav | 30,58 | 8 | 2,53 | 2,63 | 5,26 | 0,086 | 1121 |
| euh.wav | 9,14 | 8 | 2,51 | 0,83 | 3,44 | 0,090 | 876 |
| noms.wav | 5,20 | 8 | 2,52 | 0,61 | 3,23 | 0,117 | 838 |
| noms2.wav | 7,47 | 8 | 2,51 | 0,71 | 3,31 | 0,094 | 859 |
| silence.wav | 4,00 | 8 | 2,55 | 0,40 | 3,04 | 0,100 | 827 |
| bruit-piece.wav | 5,00 | 8 | 2,57 | 0,45 | 3,12 | 0,091 | 836 |
| clic.wav | 2,05 | 8 | 2,52 | 0,36 | 2,97 | 0,175 | 809 |

Chargement du modèle : **2,51 – 2,59 s** (moyenne 2,53 s sur les 20 runs), très stable. C'est un coût fixe
par lancement : sur `clic.wav` (2,05 s d'audio) il représente 91 % du temps wall.

`long.wav`, 3 runs (inférence en s) :

| Threads | run 1 | run 2 | run 3 | médiane | RTF médian |
|---|---:|---:|---:|---:|---:|
| 4 | 2,27 | 2,54 | 2,22 | 2,27 | 0,074 |
| 8 | 2,63 | 2,75 | 2,80 | 2,75 | 0,090 |

## 5. Whisper large-v3-turbo q5_0 (whisper.cpp)

`whisper-cli` imprime `load time = X ms` et `total time = X ms`, plus le détail `encode time` / `decode time`.
Le RTF est calculé sur `total time`.

| Fichier | Durée audio (s) | Threads | Chargement (s) | Inférence (s) | Wall (s) | RTF | RSS max (MB) |
|---|---:|---:|---:|---:|---:|---:|---:|
| court.wav | 7,97 | 4 | 0,26 | 21,46 | 21,52 | 2,694 | 793 |
| long.wav | 30,58 | 4 | 0,47 | 45,48 | 45,55 | 1,487 | 796 |
| euh.wav | 9,14 | 4 | 0,26 | 21,70 | 21,76 | 2,374 | 793 |
| noms.wav | 5,20 | 4 | 0,27 | 21,71 | 21,77 | 4,173 | 793 |
| noms2.wav | 7,47 | 4 | 0,27 | 21,86 | 21,92 | 2,924 | 793 |
| silence.wav | 4,00 | 4 | 0,27 | 20,90 | 20,96 | 5,224 | 792 |
| bruit-piece.wav | 5,00 | 4 | 0,26 | 20,84 | 20,90 | 4,167 | 792 |
| clic.wav | 2,05 | 4 | 0,27 | 20,93 | 20,99 | 10,208 | 792 |
| court.wav | 7,97 | 8 | 0,26 | 21,12 | 21,18 | 2,651 | 794 |
| long.wav | 30,58 | 8 | 0,26 | 45,37 | 45,43 | 1,483 | 797 |
| euh.wav | 9,14 | 8 | 0,27 | 29,82 | 29,89 | 3,262 | 794 |
| noms.wav | 5,20 | 8 | 0,27 | 31,70 | 31,78 | 6,094 | 793 |
| noms2.wav | 7,47 | 8 | 0,31 | 25,12 | 25,19 | 3,361 | 793 |
| silence.wav | 4,00 | 8 | 0,29 | 21,09 | 21,16 | 5,272 | 791 |
| bruit-piece.wav | 5,00 | 8 | 0,29 | 20,77 | 20,85 | 4,154 | 793 |
| clic.wav | 2,05 | 8 | 0,28 | 20,84 | 20,92 | 10,165 | 792 |

### 5.1 Le coût est fixe : 30 s de padding

Whisper traite toujours des fenêtres de 30 s, même pour 2 s d'audio. Conséquence directe : à 4 threads,
l'inférence est quasi constante à **20,8 – 21,9 s** pour tous les fichiers de moins de 30 s, quel que soit
leur contenu (y compris le silence). Le `encode time` seul pèse **20,43 – 20,60 s par fenêtre** à 4 threads.
`long.wav` (30,58 s) déborde sur une deuxième fenêtre : 2 × 20,56 s d'encodage, soit 45,48 s au total.

### 5.2 Variance à 8 threads

À 8 threads, `long.wav` donne 45,37 / 45,56 / **56,63** s : le 3ᵉ run est un outlier à +24 %.
Les fichiers courts à 8 threads montent aussi jusqu'à 29,8 s (`euh.wav`) et 31,7 s (`noms.wav`) contre
21,5 – 21,9 s à 4 threads. Les médianes du tableau masquent cette dispersion ; les valeurs run par run sont
dans `docs/bench-raw/logs/whisper_*_t8_r*.log`. Cause probable : 8 threads logiques sur 4 cœurs physiques,
sur une machine déjà chargée (qbittorrent + 2 JVM + whisper-server).

`long.wav`, 3 runs (inférence en s) :

| Threads | run 1 | run 2 | run 3 | médiane | RTF médian |
|---|---:|---:|---:|---:|---:|
| 4 | 45,48 | 45,44 | 45,49 | 45,48 | 1,487 |
| 8 | 45,37 | 45,56 | 56,63 | 45,56 | 1,490 |

## 6. Transcriptions

### 6.1 Côte à côte (4 threads)

| Fichier | Parakeet | Whisper |
|---|---|---|
| **court.wav** | Salut, je teste la nouvelle application de Dicté. Est-ce que ça marche bien? | Salut, je teste la nouvelle application de dictée. Est-ce que ça marche bien ? |
| **euh.wav** | Alors je voulais te dire que la réunion de demain est décalée à 15 heures, enfin non, à 16 heures. | Alors je voulais te dire que la réunion de demain est décalée à 15h, enfin non, à 16h. |
| **noms.wav** | Je lance le serveur Gexilax sur ce computé, puis j'ouvre Estel et FreeLLM API. | Je lance le serveur Gexilax sur ce computé, puis j'ouvre Estelle et Free LLM API. |
| **noms2.wav** | Ce soir je regarde un film sur Gélifin, ensuite je règle Taille Scal et je demande à Claude Code de corriger le bug. | Ce soir je regarde un film sur Jellyfin, ensuite je règle taille scale et je demande à Claude Cote de corriger le bug. |
| **long.wav** | Ceci est un test de dictée plus long pour vérifier que le découpage fonctionne correctement. Le moteur partagé ne couvre qu'environ 15 secondes d'audio par passe, donc l'application doit couper l'enregistrement en plusieurs morceaux, dans les creux de la voix, puis recoller les textes dans le bon ordre. Si tout se passe bien, cette phrase apparaîtra en entier, sans mot perdu au milieu, et sans répétition à la jonction des morceaux. Voilà, c'est la fin du test. | Ceci est un test de dictée plus longue pour vérifier que le découpage fonctionne correctement. Le moteur partagé ne couvre qu'environ 15 secondes d'audio par passe, donc l'application doit couper l'enregistrement en plusieurs morceaux, dans les creux de la voix, puis recoller les textes dans le bon ordre. Si tout se passe bien, cette phrase apparaîtra en entier, sans mots perdus au milieu, et sans répétition à la jonction des morceaux. Voilà, c'est la fin du test. Merci. |

Les 3 runs de `long.wav` donnent un texte **identique caractère pour caractère** pour chaque moteur, à 4 comme
à 8 threads ; tous les autres fichiers donnent aussi le même texte à 4 et à 8 threads. La sortie est
déterministe, seule la durée varie.

### 6.2 Écarts observés sur les mêmes fichiers

| Fichier | Parakeet | Whisper |
|---|---|---|
| court.wav | « Dicté » (accent et capitale) | « dictée » |
| euh.wav | « 15 heures », « 16 heures » (en lettres) | « 15h », « 16h » |
| noms.wav | « Estel » | « Estelle » |
| noms.wav | « FreeLLM API » | « Free LLM API » |
| noms2.wav | « Gélifin » | « Jellyfin » |
| noms2.wav | « Taille Scal » | « taille scale » |
| noms2.wav | « Claude Code » | « Claude Cote » |
| long.wav | « plus long », « sans mot perdu » | « plus longue », « sans mots perdus » |
| long.wav | pas de mot final | ajoute « Merci. » (absent de l'audio) |

Aucun des deux ne restitue exactement les noms propres des fichiers `noms*.wav` ; les deux écrivent
« computé ». Parakeet restitue mieux deux noms du jargon (« Claude Code », « FreeLLM API »), Whisper mieux
deux autres (« Jellyfin », « Estelle »).

### 6.3 Non-parole

| Fichier | Durée | Parakeet | Whisper |
|---|---:|---|---|
| silence.wav | 4,00 s | *(chaîne vide, aucun token)* | `...` |
| bruit-piece.wav | 5,00 s | *(chaîne vide, aucun token)* | `...` |
| clic.wav | 2,05 s | *(chaîne vide, aucun token)* | `Merci d'avoir regardé cette vidéo !` |

Parakeet renvoie une chaîne vide sur les trois. Whisper renvoie des points de suspension sur le silence et le
bruit, et **hallucine une phrase entière** (« Merci d'avoir regardé cette vidéo ! ») sur le simple clic de 2 s.
C'est cohérent avec le « Merci. » ajouté à la fin de `long.wav`.

## 7. Conclusion

1. Sur `long.wav` (30,58 s), Parakeet prend **2,27 s** d'inférence à 4 threads (**RTF 0,074**) ; Whisper prend **45,48 s** (**RTF 1,487**) — facteur **20,0**.
2. Sur les 8 fichiers, une passe chacun à 4 threads : **5,23 s** d'inférence pour Parakeet contre **194,87 s** pour Whisper — facteur **37,3**. Bout en bout (wall, chargement compris) : **26,2 s** contre **195,4 s**, facteur **7,5**, car Parakeet repaie 2,53 s de chargement à chaque lancement.
3. Whisper a un coût plancher de **20,8 s** par fichier (padding 30 s) : RTF **10,208** sur `clic.wav` (2,05 s) et **5,224** sur `silence.wav` (4,00 s). Le RTF de Parakeet ne dépasse jamais **0,175** (pire cas : `clic.wav` à 8 threads).
4. Pic RSS : **806 – 1122 MB** pour Parakeet (maximum sur `long.wav`), **791 – 797 MB** pour Whisper ; chargement du modèle **2,53 s** contre **0,28 s** en moyenne.
5. Passer de 4 à 8 threads n'apporte rien : `long.wav` fait **2,75 s** contre **2,27 s** chez Parakeet (+21 %) et **45,56 s** contre **45,48 s** chez Whisper, avec un outlier à **56,63 s**.

## 8. Fichiers bruts

Tout est dans `docs/bench-raw/` :

- `parakeet_results.json`, `whisper_results.json` — les 20 runs de chaque moteur, avec durée audio,
  threads, wall, pic RSS, chargement, inférence, RTF, texte et code de sortie.
- `logs/parakeet_<fichier>_t<N>_r<rep>.log` et `logs/whisper_<fichier>_t<N>_r<rep>.log` — stdout et stderr
  complets de chaque run (les timings de sherpa-onnx et de whisper.cpp y sont en clair), 42 fichiers au total.
- `logs/parakeet-warmup.log`, `logs/whisper-warmup.log` — runs de chauffe, non comptés.

Les copies de `docs/bench-raw/` sont identiques (md5) aux originaux laissés sur le serveur dans
`/home/ppocass/bench-plume/`.
