//! Les trois effets de bord réels du chef d'orchestre : le micro (`Mic`),
//! le moteur — Parakeet **ou** Whisper (chargé en tâche de fond) — et le
//! collage au curseur.
//!
//! Rien de spécifique à la dictée ici : chaque type ne fait que brancher un
//! trait du module sur le module correspondant (`input::mic`, `engine`,
//! `input::paste`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};

use super::{Paster, Recorder, Transcriber, Transcribed};
use crate::audio::gate::{self, GateVerdict};
use crate::engine::{transcribe_dictation, Engine, Family};
use crate::input::mic::Mic;

/// Journal de collage pour les tests : quand cette variable d'environnement est
/// posée, chaque texte collé est **aussi** ajouté à ce fichier (une ligne par
/// texte). C'est le point d'observation de `tests/e2e_x11.sh` : il permet de
/// vérifier « touche → parole → texte collé » sans fenêtre qui reçoive le
/// Ctrl+V.
pub const PASTE_LOG_ENV: &str = "PLUME_PASTE_LOG";

// MARK: Micro

/// Le micro du système (ou le fichier de `PLUME_FAKE_AUDIO`), éventuellement
/// celui choisi dans les réglages.
pub struct SystemRecorder {
    /// Ouvert au début d'une dictée, fermé à la fin : rien n'écoute entre deux.
    mic: Option<Mic>,
    /// Le nom du micro choisi (`Prefs.micDevice`) ; `None` = celui du système.
    device: Option<String>,
}

impl Default for SystemRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemRecorder {
    pub fn new() -> Self {
        Self {
            mic: None,
            device: None,
        }
    }

    /// Le micro choisi par son nom ; s'il a disparu, `start()` le dit.
    pub fn with_device(device: Option<String>) -> Self {
        Self {
            mic: None,
            device: device.filter(|name| !name.trim().is_empty()),
        }
    }
}

impl Recorder for SystemRecorder {
    fn start(&mut self) -> Result<()> {
        let mut mic = match &self.device {
            Some(name) => Mic::with_device(name.clone()),
            None => Mic::new(),
        };
        mic.start()?;
        self.mic = Some(mic);
        Ok(())
    }

    fn stop(&mut self) -> Vec<f32> {
        self.mic
            .take()
            .map(|mut mic| mic.stop())
            .unwrap_or_default()
    }

    fn level(&self) -> f32 {
        self.mic.as_ref().map(Mic::level).unwrap_or(0.0)
    }
}

// MARK: Moteur

/// Le moteur (Parakeet ou Whisper), chargé une fois et gardé en mémoire tant
/// qu'on dicte.
///
/// Le nom `ParakeetTranscriber` est conservé pour les appelants existants :
/// c'est le même type, qui sait maintenant charger l'une ou l'autre famille.
pub struct EngineTranscriber {
    model_dir: PathBuf,
    threads: i32,
    /// La langue parlée, pour Whisper (`auto` = détection).
    language: String,
    state: Mutex<EngineState>,
    /// Réveille `transcribe` quand le chargement de fond est fini.
    ready: Condvar,
}

#[derive(Default)]
struct EngineState {
    engine: Option<Engine>,
    /// Un chargement est en cours (dans un autre fil).
    loading: bool,
    /// Une mise en veille a été demandée pendant le chargement : on ne garde
    /// pas le modèle pour le relâcher aussitôt.
    release_pending: bool,
    /// La dernière erreur de chargement (relue par `transcribe`).
    error: Option<String>,
}

/// Un moteur est chargé quelque part dans l'app (pour `engine_status`).
static ENGINE_LOADED: AtomicBool = AtomicBool::new(false);

/// La génération du chef d'orchestre en cours : une relance des réglages en
/// ouvre une nouvelle, et l'ancienne boucle s'arrête à la première touche.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Capture d'une touche en cours (Réglages) : le geste de dictée est ignoré.
static CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

impl EngineTranscriber {
    /// Le dossier du modèle.
    pub fn new(model_dir: impl Into<PathBuf>, threads: i32) -> Self {
        Self::with_language(model_dir, threads, "auto")
    }

    /// Les mêmes réglages, avec la langue parlée de Whisper.
    pub fn with_language(
        model_dir: impl Into<PathBuf>,
        threads: i32,
        language: impl Into<String>,
    ) -> Self {
        Self {
            model_dir: model_dir.into(),
            threads,
            language: language.into(),
            state: Mutex::new(EngineState::default()),
            ready: Condvar::new(),
        }
    }

    /// Le dossier du modèle.
    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }

    /// Un moteur est-il chargé en mémoire (pour `engine_status`) ?
    pub fn engine_loaded() -> bool {
        ENGINE_LOADED.load(Ordering::SeqCst)
    }

    /// Ouvre une nouvelle génération du chef d'orchestre et rend son numéro.
    pub fn next_generation() -> u64 {
        GENERATION.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Cette génération est-elle toujours celle qui agit ?
    pub fn is_current_generation(generation: u64) -> bool {
        GENERATION.load(Ordering::SeqCst) == generation
    }

    /// Active/désactive la capture de touche (le geste de dictée est ignoré
    /// pendant la capture).
    pub fn set_capture_active(active: bool) {
        CAPTURE_ACTIVE.store(active, Ordering::SeqCst);
    }

    /// Une capture de touche est-elle en cours ?
    pub fn capture_active() -> bool {
        CAPTURE_ACTIVE.load(Ordering::SeqCst)
    }

    /// Le verrou, même si un fil a paniqué en le tenant (on ne veut pas qu'une
    /// dictée meure sur un empoisonnement).
    fn lock(&self) -> MutexGuard<'_, EngineState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Transcriber for EngineTranscriber {
    fn is_loaded(&self) -> bool {
        let state = self.lock();
        state.loading || state.engine.is_some()
    }

    fn load(&self) -> Result<()> {
        {
            let mut state = self.lock();
            if state.loading || state.engine.is_some() {
                return Ok(());
            }
            state.loading = true;
            state.error = None;
        }

        // Le chargement lui-même se fait sans tenir le verrou : `transcribe`
        // peut attendre sans bloquer un éventuel `release`. La famille vient
        // des fichiers du dossier (Parakeet : `joiner`, Whisper : `*-encoder`).
        let result = Family::detect(&self.model_dir).and_then(|family| {
            Engine::load(&self.model_dir, family, self.threads, &self.language)
        });

        let mut state = self.lock();
        state.loading = false;
        match result {
            Ok(engine) => {
                if state.release_pending {
                    state.release_pending = false;
                    ENGINE_LOADED.store(false, Ordering::SeqCst);
                } else {
                    state.engine = Some(engine);
                    ENGINE_LOADED.store(true, Ordering::SeqCst);
                }
                self.ready.notify_all();
                Ok(())
            }
            Err(error) => {
                state.error = Some(format!("{error:#}"));
                ENGINE_LOADED.store(false, Ordering::SeqCst);
                self.ready.notify_all();
                Err(error)
            }
        }
    }

    fn transcribe(&self, samples: &[f32]) -> Result<Transcribed> {
        // La barrière anti-hallucination d'abord : c'est elle qui donne la
        // raison du refus (« silence », « pas de parole », « trop court »…).
        // Un enregistrement sans vraie parole n'est JAMAIS envoyé au moteur.
        if let GateVerdict::Rejected { reason } = gate::analyse(samples, gate::SAMPLE_RATE) {
            return Ok(Transcribed::Rejected(reason));
        }

        let mut state = self.lock();
        // Une dictée a besoin du moteur : une mise en veille demandée pendant
        // le chargement est annulée (sinon on relâcherait le modèle juste
        // avant de transcrire).
        state.release_pending = false;
        while state.loading {
            state = self
                .ready
                .wait_timeout(state, Duration::from_millis(50))
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
        if let Some(error) = &state.error {
            return Err(anyhow!("moteur : {error}"));
        }
        let Some(engine) = &state.engine else {
            bail!("modèle non chargé");
        };

        // Le moteur gère lui-même les longues séquences : on découpe seulement
        // au-delà de 25 s, dans les creux de la voix.
        match transcribe_dictation(engine, samples)? {
            Some(text) if !text.trim().is_empty() => Ok(Transcribed::Text(text)),
            _ => Ok(Transcribed::Rejected("rien entendu".into())),
        }
    }

    fn release(&self) {
        let mut state = self.lock();
        if state.loading {
            // Le chargement finira : on ne garde pas le modèle, mais on ne
            // bloque pas la boucle de dictée en l'attendant.
            state.release_pending = true;
            return;
        }
        state.engine = None;
        ENGINE_LOADED.store(false, Ordering::SeqCst);
    }
}

/// L'ancien nom du moteur Parakeet, gardé pour les appelants.
pub type ParakeetTranscriber = EngineTranscriber;

// MARK: Collage

/// Le collage réel : presse-papiers + Ctrl+V (`input::paste`), plus le journal
/// de test `PLUME_PASTE_LOG`.
pub struct SystemPaster;

impl Paster for SystemPaster {
    fn paste(&self, text: &str) -> Result<()> {
        crate::input::paste::paste(text)?;
        if let Some(path) = paste_log_path() {
            if let Err(error) = append_paste_log(&path, text) {
                log::warn!(
                    "journal de collage non écrit ({}): {error}",
                    path.display()
                );
            }
        }
        Ok(())
    }
}

/// Le fichier de `PLUME_PASTE_LOG` (vide ou absent : pas de journal).
pub fn paste_log_path() -> Option<PathBuf> {
    std::env::var(PASTE_LOG_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Ajoute `text` au journal, une ligne par texte collé.
pub fn append_paste_log(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{text}")?;
    file.flush()
}

impl super::Controller<SystemRecorder, EngineTranscriber, SystemPaster> {
    /// Le chef d'orchestre branché sur le vrai micro, le vrai modèle et le vrai
    /// collage — ce que `plume-cli run` et l'app Tauri utilisent.
    pub fn system(settings: super::Settings, events: Sender<super::Event>) -> Self {
        Self::system_with_mic(settings, events, None)
    }

    /// Le même, avec le micro choisi dans les réglages (`Prefs.micDevice`).
    pub fn system_with_mic(
        settings: super::Settings,
        events: Sender<super::Event>,
        mic: Option<String>,
    ) -> Self {
        let transcriber = EngineTranscriber::with_language(
            &settings.model_dir,
            settings.threads,
            &settings.lang,
        );
        Self::new(
            settings,
            events,
            SystemRecorder::with_device(mic),
            transcriber,
            SystemPaster,
        )
    }

    /// La boucle de vie d'une génération donnée : chaque geste, plus le
    /// battement qui envoie les niveaux et relâche le moteur.
    ///
    /// Une relance des réglages (`next_generation`) rend cette boucle obsolète :
    /// elle s'arrête à la première touche, sans rien dicter — la nouvelle
    /// génération, elle, écoute avec les nouveaux réglages. Pendant une capture
    /// de touche, le geste est ignoré.
    pub fn run_generation(&mut self, generation: u64) -> Result<()> {
        let gestures = crate::input::listener::start(self.gesture_config())?;
        loop {
            if !EngineTranscriber::is_current_generation(generation) {
                return Ok(());
            }
            match gestures.recv_timeout(self.next_timeout()) {
                Ok(output) => {
                    if !EngineTranscriber::is_current_generation(generation) {
                        return Ok(());
                    }
                    if !EngineTranscriber::capture_active() {
                        self.handle(output);
                    }
                }
                Err(RecvTimeoutError::Timeout) => self.tick(),
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_paste_log_appends_one_line_per_text() {
        let dir = std::env::temp_dir().join("plume-paste-log-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("paste.log");
        std::fs::remove_file(&path).ok();

        append_paste_log(&path, "Salut").unwrap();
        append_paste_log(&path, "Deuxième ligne").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "Salut\nDeuxième ligne\n");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_missing_engine_reports_an_error_instead_of_panicking() {
        let transcriber = ParakeetTranscriber::new("/dossier/qui/n/existe/pas", 4);
        assert!(!transcriber.is_loaded());
        // Le chargement échoue proprement…
        assert!(transcriber.load().is_err());
        assert!(!transcriber.is_loaded());
        // …et une transcription le dit au lieu de paniquer.
        let samples = vec![0.3f32; 16_000];
        let error = transcriber.transcribe(&samples).unwrap_err();
        assert!(format!("{error:#}").contains("moteur"), "{error:#}");
    }

    #[test]
    fn silence_is_rejected_before_touching_the_engine() {
        let transcriber = ParakeetTranscriber::new("/dossier/qui/n/existe/pas", 4);
        let silence = vec![0.0f32; 16_000];
        // Le moteur n'est pas chargé : la barrière doit refuser sans erreur.
        assert_eq!(
            transcriber.transcribe(&silence).unwrap(),
            Transcribed::Rejected("silence".into())
        );
    }

    #[test]
    fn a_generation_is_current_until_the_next_one() {
        let first = EngineTranscriber::next_generation();
        assert!(EngineTranscriber::is_current_generation(first));
        let second = EngineTranscriber::next_generation();
        assert!(!EngineTranscriber::is_current_generation(first));
        assert!(EngineTranscriber::is_current_generation(second));
    }

    #[test]
    fn capture_is_a_flag_around_the_gesture() {
        assert!(!EngineTranscriber::capture_active());
        EngineTranscriber::set_capture_active(true);
        assert!(EngineTranscriber::capture_active());
        EngineTranscriber::set_capture_active(false);
        assert!(!EngineTranscriber::capture_active());
    }

    #[test]
    fn the_microphone_can_be_chosen_by_name() {
        let recorder = SystemRecorder::with_device(Some("  ".to_string()));
        assert!(recorder.device.is_none(), "un nom vide = micro du système");
        let recorder = SystemRecorder::with_device(Some("Blue Yeti".to_string()));
        assert_eq!(recorder.device.as_deref(), Some("Blue Yeti"));
    }
}
