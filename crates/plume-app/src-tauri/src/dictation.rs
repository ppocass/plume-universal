//! La dictée dans l'app : le chef d'orchestre de `plume-core` tourne dans un
//! fil, ses événements deviennent des événements Tauri (`plume://phase`,
//! `plume://level`), la pastille suit les phases, et chaque dictée réussie
//! entre dans l'historique — les mêmes champs que l'app macOS.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use chrono::Utc;
use plume_core::controller::{channel, Controller, Event, ParakeetTranscriber, Phase, Settings};
use plume_core::store::prefs::{DictationLanguage, LeetLevel, PolishService};
use plume_core::store::{paths, Dictation, Prefs};
use plume_core::text::{Level, PolishConfig};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::state::AppState;

/// Les durées d'affichage de la pastille (`DictationController.swift`) :
/// collé 0,9 s, message 1,6 s, annulé tout de suite.
pub const DONE_MS: u64 = 900;
pub const MESSAGE_MS: u64 = 1_600;

/// Ce que fait la pastille pour une phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PillAction {
    Show,
    Keep,
    Hide(Duration),
}

/// L'état de la pastille pour une phase (`§3.2` et `§3.10` de la spéc).
pub fn pill_action(phase: &Phase) -> PillAction {
    match phase {
        Phase::Recording => PillAction::Show,
        // Le traitement garde la pastille affichée : elle passe juste en ambre.
        Phase::Processing => PillAction::Keep,
        Phase::Done { .. } => PillAction::Hide(Duration::from_millis(DONE_MS)),
        Phase::Cancelled => PillAction::Hide(Duration::ZERO),
        Phase::Rejected(_) | Phase::Error(_) => {
            PillAction::Hide(Duration::from_millis(MESSAGE_MS))
        }
        Phase::Idle => PillAction::Hide(Duration::ZERO),
    }
}

/// Le nom de phase envoyé au front, en minuscules (`plume://phase`).
pub fn phase_name(phase: &Phase) -> &'static str {
    match phase {
        Phase::Idle => "idle",
        Phase::Recording => "recording",
        Phase::Processing => "processing",
        Phase::Done { .. } => "done",
        Phase::Cancelled => "cancelled",
        Phase::Rejected(_) => "rejected",
        Phase::Error(_) => "error",
    }
}

/// Le texte facultatif qui accompagne la phase : la raison d'un refus, ou le
/// texte transcrit quand la dictée est finie.
pub fn phase_text(phase: &Phase, transcript: Option<&str>) -> Option<String> {
    match phase {
        Phase::Rejected(reason) | Phase::Error(reason) => Some(reason.clone()),
        Phase::Done { .. } => transcript.map(ToString::to_string),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhasePayload {
    pub phase: &'static str,
    pub text: Option<String>,
}

// MARK: Réglages du chef d'orchestre

/// Les réglages du chef d'orchestre, construits depuis `Prefs`.
pub fn settings_from_prefs(prefs: &Prefs) -> Settings {
    let mut settings = Settings::new(model_dir_for(prefs));
    settings.key = prefs.dictation_key();
    settings.leet_key = Some(prefs.leet_key());
    // `-1` = jamais arrêté ; une année tient lieu d'infini sans risquer un
    // dépassement d'`Instant`.
    settings.idle_release_secs = if prefs.idle_seconds < 0.0 {
        31_536_000
    } else {
        prefs.idle_seconds as u64
    };
    settings.polish = polish_from_prefs(prefs);
    // Le même fichier que `get_vocabulary` / `set_vocabulary` : les mots
    // ajoutés dans les réglages s'appliquent à la dictée suivante.
    settings.vocabulary = Some(crate::extra::vocabulary_path());
    settings.lang = match prefs.language {
        DictationLanguage::Auto => "auto",
        DictationLanguage::Fr => "fr",
        DictationLanguage::En => "en",
        DictationLanguage::Es => "es",
        DictationLanguage::De => "de",
        DictationLanguage::It => "it",
        DictationLanguage::Pt => "pt",
    }
    .to_string();
    settings.leet_level = match prefs.leet_level {
        LeetLevel::Light => Level::Light,
        LeetLevel::Medium => Level::Medium,
        LeetLevel::Full => Level::Full,
    };
    settings
}

/// `<données>/models/<prefs.model>` : le modèle choisi (Parakeet par défaut).
pub fn model_dir_for(prefs: &Prefs) -> PathBuf {
    paths::models_dir().join(&prefs.model)
}

/// Deux réglages demandent-ils une relance du chef d'orchestre ?
///
/// Ce sont exactement les champs que `settings_from_prefs` transmet au cœur :
/// touche de dictée, modificateur 1337, langue, modèle, lissage, veille,
/// niveau 1337 et micro.
pub fn needs_reload(previous: &Prefs, next: &Prefs) -> bool {
    previous.dictation_key != next.dictation_key
        || previous.leet_key != next.leet_key
        || previous.leet_level != next.leet_level
        || previous.language != next.language
        || previous.model != next.model
        || previous.mic_device != next.mic_device
        || previous.idle_seconds != next.idle_seconds
        || previous.polish_enabled != next.polish_enabled
        || previous.polish_service != next.polish_service
        || previous.polish_custom_url != next.polish_custom_url
        || previous.polish_model != next.polish_model
        || previous.polish_api_key != next.polish_api_key
}

/// Le lissage, seulement quand il y a de quoi l'appeler (service, adresse,
/// modèle et clé) : sinon le texte brut part tel quel.
fn polish_from_prefs(prefs: &Prefs) -> Option<PolishConfig> {
    if !prefs.polish_enabled {
        return None;
    }
    let base_url = match prefs.polish_service {
        PolishService::Groq => "https://api.groq.com/openai/v1".to_string(),
        PolishService::OpenAi => "https://api.openai.com/v1".to_string(),
        PolishService::Server | PolishService::Custom => prefs.polish_custom_url.trim().to_string(),
    };
    let model = prefs.polish_model.trim();
    let key = prefs.polish_api_key.trim();
    if base_url.is_empty() || model.is_empty() || key.is_empty() {
        return None;
    }
    Some(PolishConfig::new(base_url, model, Some(key.to_string())))
}

// MARK: Le fil de la dictée

/// Démarre le chef d'orchestre et le fil qui relaie ses événements.
pub fn spawn(app: AppHandle, prefs: &Prefs) {
    let settings = settings_from_prefs(prefs);
    let mic = prefs.mic_device.clone();
    let has_model = std::fs::read_dir(&settings.model_dir)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .any(|entry| {
                    entry
                        .path()
                        .extension()
                        .map(|ext| ext.to_string_lossy() == "onnx")
                        .unwrap_or(false)
                })
        })
        .unwrap_or(false);
    if !has_model {
        // Pas de modèle : l'app reste ouverte, la dictée dira simplement
        // qu'elle ne peut pas transcrire.
        log::warn!(
            "modèle absent ({}) : l'app reste ouverte",
            settings.model_dir.display()
        );
    }

    let generation = ParakeetTranscriber::next_generation();
    let (sender, receiver) = channel();
    let spawned = std::thread::Builder::new()
        .name("plume-dictee".into())
        .spawn(move || {
            let mut controller = Controller::system_with_mic(settings, sender, mic);
            if let Err(error) = controller.run_generation(generation) {
                log::warn!("écoute de la dictée arrêtée : {error:#}");
            }
        });
    if let Err(error) = spawned {
        log::error!("fil de la dictée : {error}");
        return;
    }

    if let Err(error) = std::thread::Builder::new()
        .name("plume-evenements".into())
        .spawn(move || forward(app, receiver))
    {
        log::error!("fil des événements : {error}");
    }
}

/// Relance le chef d'orchestre avec de nouveaux réglages, sans redémarrer
/// l'app : l'ancienne boucle s'arrête à la première touche (elle n'agit plus),
/// et la nouvelle écoute avec les réglages à jour.
pub fn reload(app: AppHandle, prefs: &Prefs) {
    log::info!("réglages de dictée modifiés : relance du chef d'orchestre");
    spawn(app, prefs);
}

/// Relaie les événements du chef d'orchestre vers le front et la pastille.
fn forward(app: AppHandle, events: Receiver<Event>) {
    let mut transcript: Option<String> = None;
    let mut recording_started: Option<Instant> = None;
    // La durée d'audio, arrêtée à la fin de l'enregistrement (le traitement
    // qui suit ne fait pas partie de l'audio).
    let mut recorded_seconds = 0.0f64;

    for event in events {
        match event {
            Event::Level(level) => {
                let _ = app.emit("plume://level", level);
            }
            Event::Text(text) => {
                transcript = Some(text);
            }
            Event::Phase(phase) => {
                if phase == Phase::Recording {
                    recording_started = Some(Instant::now());
                    recorded_seconds = 0.0;
                    transcript = None;
                }
                if phase == Phase::Processing {
                    recorded_seconds = recording_started
                        .take()
                        .map(|started| started.elapsed().as_secs_f64())
                        .unwrap_or(0.0);
                }
                let text = phase_text(&phase, transcript.as_deref());
                let payload = PhasePayload {
                    phase: phase_name(&phase),
                    text,
                };
                let _ = app.emit("plume://phase", &payload);

                if let Phase::Done { pasted } = &phase {
                    if let Some(text) = transcript.take() {
                        add_history(&app, new_dictation(text, *pasted, recorded_seconds));
                    }
                }

                follow_pill(&app, &phase);
            }
        }
    }
}

/// Montre, garde ou cache la pastille, en respectant les durées de la spéc.
fn follow_pill(app: &AppHandle, phase: &Phase) {
    match pill_action(phase) {
        PillAction::Show => show_pill(app),
        PillAction::Keep => {}
        PillAction::Hide(delay) => hide_pill_after(app.clone(), delay),
    }
}

/// Compteur de génération : un « cacher » programmé ne doit pas éteindre une
/// pastille qu'une nouvelle dictée vient de rallumer.
static PILL_GENERATION: AtomicU64 = AtomicU64::new(0);

fn show_pill(app: &AppHandle) {
    PILL_GENERATION.fetch_add(1, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("pill") {
        let _ = window.show();
    }
}

fn hide_pill(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("pill") {
        let _ = window.hide();
    }
}

fn hide_pill_after(app: AppHandle, delay: Duration) {
    let generation = PILL_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::Builder::new()
        .name("plume-pastille".into())
        .spawn(move || {
            if !delay.is_zero() {
                std::thread::sleep(delay);
            }
            if PILL_GENERATION.load(Ordering::SeqCst) == generation {
                hide_pill(&app);
            }
        })
        .ok();
}

// MARK: L'historique

/// Une entrée d'historique avec les champs de l'app macOS.
pub fn new_dictation(text: String, pasted: bool, audio_seconds: f64) -> Dictation {
    Dictation {
        id: Uuid::new_v4(),
        date: Utc::now(),
        raw: text.clone(),
        text,
        // Le nettoyage a pu échouer sans qu'on le sache ici : on ne prétend
        // pas que le texte est nettoyé.
        polished: false,
        pasted,
        app_name: None,
        audio_seconds,
        failed: false,
        audio_file: None,
        leet: false,
    }
}

fn add_history(app: &AppHandle, dictation: Dictation) {
    let state = app.state::<AppState>();
    state.history().add(dictation);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictation_settings_depuis_prefs() {
        let prefs = Prefs::default();
        let settings = settings_from_prefs(&prefs);
        assert_eq!(settings.key, prefs.dictation_key());
        assert_eq!(settings.leet_level, Level::Medium);
        assert_eq!(settings.lang, "auto");
        assert_eq!(settings.idle_release_secs, 35);
        assert!(settings.model_dir.ends_with("models/parakeet-v3"));
        assert!(settings.vocabulary.is_some());
        assert!(settings.polish.is_none(), "pas de clé : pas de lissage");
    }

    #[test]
    fn dictation_settings_suivent_les_prefs() {
        let mut prefs = Prefs::default();
        prefs.idle_seconds = -1.0;
        prefs.language = DictationLanguage::Fr;
        prefs.leet_level = LeetLevel::Full;
        prefs.polish_enabled = false;
        let settings = settings_from_prefs(&prefs);
        assert_eq!(settings.idle_release_secs, 31_536_000, "-1 = jamais arrêté");
        assert_eq!(settings.lang, "fr");
        assert_eq!(settings.leet_level, Level::Full);
        assert!(settings.polish.is_none());
    }

    #[test]
    fn dictation_model_dir_suit_le_modele_choisi() {
        let mut prefs = Prefs::default();
        assert!(model_dir_for(&prefs).ends_with("models/parakeet-v3"));
        prefs.model = "whisper-base".into();
        assert!(model_dir_for(&prefs).ends_with("models/whisper-base"));
    }

    #[test]
    fn dictation_needs_reload_only_for_live_settings() {
        let prefs = Prefs::default();
        assert!(!needs_reload(&prefs, &prefs), "rien de changé");

        let mut key = prefs.clone();
        key.dictation_key = "f9".into();
        assert!(needs_reload(&prefs, &key));

        let mut model = prefs.clone();
        model.model = "whisper-small".into();
        assert!(needs_reload(&prefs, &model));

        let mut mic = prefs.clone();
        mic.mic_device = Some("Blue Yeti".into());
        assert!(needs_reload(&prefs, &mic));

        let mut polish = prefs.clone();
        polish.polish_api_key = "sk-test".into();
        assert!(needs_reload(&prefs, &polish));

        let mut idle = prefs.clone();
        idle.idle_seconds = 120.0;
        assert!(needs_reload(&prefs, &idle));

        // L'apparence ou la langue de l'interface ne relancent pas la dictée.
        let mut look = prefs.clone();
        look.appearance = plume_core::store::prefs::AppAppearance::Dark;
        look.app_language = plume_core::store::prefs::AppLanguage::Fr;
        assert!(!needs_reload(&prefs, &look));
    }

    #[test]
    fn dictation_phases_et_durees_de_la_pastille() {
        assert_eq!(phase_name(&Phase::Recording), "recording");
        assert_eq!(phase_name(&Phase::Done { pasted: true }), "done");
        assert_eq!(
            phase_text(&Phase::Rejected("silence".into()), None),
            Some("silence".to_string())
        );
        assert_eq!(
            phase_text(&Phase::Done { pasted: true }, Some("bonjour")),
            Some("bonjour".to_string())
        );
        assert_eq!(pill_action(&Phase::Recording), PillAction::Show);
        assert_eq!(pill_action(&Phase::Processing), PillAction::Keep);
        assert_eq!(
            pill_action(&Phase::Done { pasted: true }),
            PillAction::Hide(Duration::from_millis(DONE_MS))
        );
        assert_eq!(
            pill_action(&Phase::Cancelled),
            PillAction::Hide(Duration::ZERO)
        );
        assert_eq!(
            pill_action(&Phase::Rejected("bruit".into())),
            PillAction::Hide(Duration::from_millis(MESSAGE_MS))
        );
    }

    #[test]
    fn dictation_entree_historique_comme_swift() {
        let entry = new_dictation("Bonjour le monde".into(), true, 2.5);
        assert_eq!(entry.text, "Bonjour le monde");
        assert_eq!(entry.raw, entry.text);
        assert!(entry.pasted);
        assert!(!entry.polished);
        assert!(!entry.failed);
        assert!(!entry.leet);
        assert_eq!(entry.audio_seconds, 2.5);
        assert_eq!(entry.word_count(), 3);
        assert!(entry.app_name.is_none());
        assert!(entry.audio_file.is_none());
    }

    #[test]
    fn dictation_lissage_seulement_si_complet() {
        let mut prefs = Prefs::default();
        prefs.polish_service = PolishService::Groq;
        prefs.polish_model = "openai/gpt-oss-120b".into();
        assert_eq!(polish_from_prefs(&prefs), None, "clé absente");

        prefs.polish_api_key = "sk-test".into();
        let polish = polish_from_prefs(&prefs).expect("lissage complet");
        assert_eq!(polish.base_url, "https://api.groq.com/openai/v1");
        assert_eq!(polish.model, "openai/gpt-oss-120b");
        assert_eq!(polish.api_key.as_deref(), Some("sk-test"));
    }
}
