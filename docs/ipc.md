# Contrat interface ↔ Rust (Plume v2) — fixé par Vera, ne pas changer sans elle

Appels : `invoke(nom, args)` de `ui/js/tauri.js`. Événements : `listen(nom, cb)`, charge utile dans `event.payload`.
Les noms d'arguments sont en camelCase côté JS (Tauri convertit). Toutes les erreurs = chaîne lisible.

## Déjà là (coquille)
get_prefs → Prefs · set_prefs(patch) → Prefs · get_history → Dictation[] · delete_dictation(id) · clear_history ·
totals → {today, last7Days, total} · app_info → AppInfo · open_window(name) · quit · ui_env

## Nouveaux (src-tauri/src/extra.rs)
| Commande | Arguments | Retour |
|---|---|---|
| system_check | — | `{ sessionType: "x11"\|"wayland"\|"windows"\|"other", pasteTool: bool, pasteToolName: string, micCount: number }` |
| list_mics | — | `string[]` (noms ; `""` n'est jamais renvoyé) |
| mic_test_start | `{ device: string\|null }` | — ; émet `plume://mic-level` `{ level: 0..1 }` ~20/s |
| mic_test_stop | — | — |
| list_models | — | `ModelView[]` |
| recommend_model | `{ spokenLanguage: string }` | `{ id, reasonKey, machine: { cpu, physicalCores, ramTotalGb, ramAvailableGb, avx2, nvidia } }` |
| download_model | `{ id }` | — ; émet `plume://model-progress` `{ id, doneBytes, totalBytes }`, puis `plume://model-done` `{ id, ok, error\|null }` |
| cancel_download | `{ id }` | — |
| delete_model | `{ id }` | — |
| use_model | `{ id }` | — (enregistre le choix, recharge le moteur) |
| engine_status | — | `{ state: "missing"\|"downloading"\|"ready"\|"loaded", modelId }` ; changements émis en `plume://engine-status` (même forme) |
| get_vocabulary | — | Vocabulary (JSON du cœur, mêmes champs que Swift) |
| set_vocabulary | `{ vocabulary }` | — |
| leet_convert | `{ text, level: number, reverse: bool }` | string |
| rewrite_dictation | `{ id, toLeet: bool }` | Dictation |
| retry_dictation | `{ id }` | Dictation (re-transcrit l'audio gardé) |
| copy_text | `{ text }` | — |
| polish_test | `{ baseUrl, model, apiKey }` | `{ ok, ms, sample, error\|null }` |
| set_dictation_key_capture | `{ on: bool }` | — ; quand on : émet `plume://key-captured` `{ name }` à la prochaine touche seule |
| get_whats_new | — | `{ version, items: [{ titleKey, bodyKey }] }` |
| mark_whats_new_seen | — | — |

`ModelView` = `{ id, family: "parakeet"\|"whisper"\|"cloud", name, sizeMb, languages: string, speed: 1..5,
accuracy: 1..5, descriptionKey, installed: bool, downloading: bool, progress: 0..1, recommended: bool, active: bool }`

## Catalogue (Linux/Windows)
- `parakeet-v3` : Parakeet TDT 0.6B v3 int8 (sherpa-onnx), ~640 Mo, 25 langues européennes, vitesse 5, précision 5.
  Défaut. Mesuré : RTF 0,074 sur i7-6700 (docs/bench-zcomputer.md).
- `whisper-small` et `whisper-base` : Whisper ONNX de sherpa-onnx (`sherpa-onnx-whisper-small`, `-base`), pour les langues
  hors des 25. Moins justes en français que Parakeet ; à mesurer.
- `cloud` : « En ligne » (API compatible OpenAI, clé de l'utilisateur) — plus tard, affiché « bientôt ».

## Règle de recommandation (transposée du Mac, 16/10)
Langue parlée dans les 25 de Parakeet → `parakeet-v3`, sauf si RAM disponible < 1,5 Go → `whisper-base`.
Autre langue → `whisper-small` si RAM totale ≥ 8 Go et disponible ≥ 2 Go, sinon `whisper-base`.
Relevée **une fois** (au premier calcul, enregistrée dans les réglages), comme sur Mac.
