# Retouches relevées par Vera sur les captures (à faire en une passe finale)

Tout est fait. Chaque ligne porte `[fait]` et la façon dont le point a été vérifié.
Captures : `docs/shots/fx/` (produites par `tests/shot_fx.sh` dans le conteneur de build).

- [fait] **Touches avec les symboles Mac** (« Left ⌥ ») → un seul module partagé,
  `crates/plume-app/ui/js/keys.js` (`keyLabel`, `keyChoices`), utilisé par Accueil,
  Historique, Réglages, 1337 et l'assistant. Les libellés viennent de `ui/i18n/{en,fr}.json`
  (`key_left_alt` = « Left Alt » / « Alt gauche », `key_super` = « Super », `key_right_ctrl`…,
  `F1`…`F24` tels quels). Vérifié sur `docs/shots/fx/home-light.png` (« Left Alt » sur les trois
  gestes), `home-light-fr.png` (« Alt gauche ») et `settings-light.png` (raccourci « Left Alt »).
  Un test Rust (`les_libelles_de_touche_sont_ceux_de_linux`) refuse tout symbole `⌥ ⌘ ⌃ ⇧`.

- [fait] **Textes restés « Mac » et « Neural Engine »** → plus aucun. « This Mac » → « This
  computer » / « Cet ordinateur » ; « Recommended for this Mac » → « … for this computer » ;
  « Nothing leaves your Mac » → « … your computer » ; « your voice never leaves it » ;
  Parakeet « Runs on the processor: very fast, even without a graphics card. » ; Whisper
  « Runs on the processor, slower than Parakeet. More languages, and it uses your vocabulary. » ;
  Moteur « Runs offline, on this computer. ». Les clés encore nommées `…_mac` ont été renommées
  (`runs_on_the_neural_engine_…` → `runs_on_the_processor_very_fast_without_a_graphics_card`,
  `this_mac` → `this_computer`, etc.) dans les deux fichiers **et** dans le HTML/JS/Rust qui les
  citent. Vérifié sur `models-light.png`, `leet-light.png` et `settings-light.png` ;
  `grep 'Neural Engine'` et `grep 'this Mac|This Mac|your Mac|ce Mac|votre Mac'` sur
  `ui/i18n/en.json` et `fr.json` ne rendent plus rien ; test Rust
  `les_traductions_ne_parlent_plus_du_mac` (mêmes clés en/fr, textes interdits absents).

- [fait] **Recommandation « Automatic detection » → `parakeet-v3`** → `models::recommend` traite
  `auto` / `automatic` / `""` comme une langue de Parakeet (il détecte seul parmi ses 25 langues),
  avec le garde-fou de mémoire conservé. Vérifié sur `models-light.png` : le badge « Recommended
  for this computer » est sur **Parakeet TDT 0.6B v3** (et le pied « Model » aussi) ;
  test `recommend_rule_cases` étendu (`auto`, `""`, `Automatic` → `parakeet-v3` ;
  `auto` sous 1,5 Go de RAM → `whisper-base`).

- [fait] **Assistant, étape Modèle : « Download » → « Cancel » pendant le téléchargement** → le
  bouton prend le libellé `cancel` et appelle `cancel_download` (même comportement que Modèles),
  puis retombe sur « Download ». Vérifié sur
  `docs/shots/fx/onboarding-step6-downloading.png` : bouton « Cancel » sous la barre de
  progression, « Downloading Parakeet TDT 0.6B v3: 42% of 640 MB ».

- [fait] **Pastille « bottom » : captures entièrement noires** → nouveau module
  `crates/plume-app/src-tauri/src/pill.rs` : la fenêtre prend la taille de sa carte
  (320 × 64 + 52 × 26 de marge d'ombre, pop-up 430 × 96), se centre sur la **zone de travail** du
  moniteur (`work_area()`, barre des tâches exclue) et se pose à 14 pt au-dessus du bas
  (`§3.1`) ; deux passes de correction après `setSize` parce que GTK applique la taille de façon
  asynchrone et rognait la position sur l'ancienne hauteur (c'était la cause du hors-écran).
  Vérifié sur `bottom-light-listening.png` (carte à y = 746–780 sur 800, centrée x = 582–698,
  entièrement visible) et `bottom-dark-listening.png` (test d'acceptation : l'image n'est pas
  noire) ; tests `la_pastille_du_bas_reste_dans_la_zone_de_travail`,
  `la_taille_reelle_du_systeme_garde_la_carte_dans_l_ecran`,
  `la_zone_de_travail_decalee_est_respectee`.

- [fait] **Pastille « pasted » : barres cyan + petit ✓** → sur le Mac, `content` ne rend que
  l'image du ✓ (13 pt gras, `plumeDone`, cadre 30 × 18) : les barres et le ✕ disparaissent
  (`§3.2` / `§3.7`). Ajout de `.pill-stage[data-phase="pasted"] .pill-bars-wrap { display: none }`
  dans `ui/css/pill.css` : la capsule se resserre à 54 × 34. Vérifié sur
  `top-light-pasted.png` (capsule verte ✓ seule, 54 × 34 pt, centrée).

- [fait] **Pastille, popup « Saved to history » : fenêtre 320 px pour un slot de 430** → la
  commande Rust `pill_resize(preview)` (`commands.rs` + `pill.rs`) redimensionne la fenêtre
  (482 × 122 avec les marges) et la recentre **avant** d'afficher le pop-up, puis la ramène à
  372 × 90 ; côté bas, le bord bas reste fixe (le pop-up grandit vers le haut, `§3.1`).
  Vérifié sur `top-light-popup.png` (carte 430 × 58 pt, x = 425–855 centrée) et
  `bottom-light-popup.png` / `bottom-dark-popup.png` (bord bas à 780, la carte monte) ;
  tests `le_pop_up_du_bas_grandit_vers_le_haut_sans_bouger_le_bas`,
  `la_fenetre_ajoute_les_marges_d_ombre`.
