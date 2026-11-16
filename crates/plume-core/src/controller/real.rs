//! Les trois effets de bord réels du chef d'orchestre : le micro (`Mic`),
//! Parakeet (chargé en tâche de fond) et le collage au curseur.
//!
//! Rien de spécifique à la dictée ici : chaque type ne fait que brancher un
//! trait du module sur le module correspondant (`input::mic`, `engine`,
//! `input::paste`).

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};

use super::{Paster, Recorder, Transcriber, Transcribed};
use crate::audio::gate::{self, GateVerdict};
use crate::engine::{transcribe_dictation, Parakeet};
use crate::input::mic::Mic;

/// Journal de collage pour les tests : quand cette variable d'environnement est
/// posée, chaque texte collé est **aussi** ajouté à ce fichier (une ligne par
/// texte). C'est le point d'observation de `tests/e2e_x11.sh` : il permet de
/// vérifier « touche → parole → texte collé » sans fenêtre qui reçoive le
/// Ctrl+V.
pub const PASTE_LOG_ENV: &str = "PLUME_PASTE_LOG";

// MARK: Micro

/// Le micro du système (ou le fichier de `PLUME_FAKE_AUDIO`).
pub struct SystemRecorder {
    mic: Mic,
}

impl Default for SystemRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemRecorder {
    pub fn new() -> Self {
        Self { mic: Mic::new() }
    }
}

impl Recorder for SystemRecorder {
    fn start(&mut self) -> Result<()> {
        self.mic.start()
    }

    fn stop(&mut self) -> Vec<f32> {
        self.mic.stop()
    }

    fn level(&self) -> f32 {
        self.mic.level()
    }
}

// MARK: Moteur

/// Parakeet, chargé une fois et gardé en mémoire tant qu'on dicte.
pub struct ParakeetTranscriber {
    model_dir: PathBuf,
    threads: i32,
    state: Mutex<EngineState>,
    /// Réveille `transcribe` quand le chargement de fond est fini.
    ready: Condvar,
}

#[derive(Default)]
struct EngineState {
    engine: Option<Parakeet>,
    /// Un chargement est en cours (dans un autre fil).
    loading: bool,
    /// Une mise en veille a été demandée pendant le chargement : on ne garde
    /// pas le modèle pour le relâcher aussitôt.
    release_pending: bool,
    /// La dernière erreur de chargement (relue par `transcribe`).
    error: Option<String>,
}

impl ParakeetTranscriber {
    pub fn new(model_dir: impl Into<PathBuf>, threads: i32) -> Self {
        Self {
            model_dir: model_dir.into(),
            threads,
            state: Mutex::new(EngineState::default()),
            ready: Condvar::new(),
        }
    }

    /// Le dossier du modèle.
    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }

    /// Le verrou, même si un fil a paniqué en le tenant (on ne veut pas qu'une
    /// dictée meure sur un empoisonnement).
    fn lock(&self) -> MutexGuard<'_, EngineState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Transcriber for ParakeetTranscriber {
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
        // peut attendre sans bloquer un éventuel `release`.
        let result = Parakeet::load(&self.model_dir, self.threads);

        let mut state = self.lock();
        state.loading = false;
        match result {
            Ok(engine) => {
                if state.release_pending {
                    state.release_pending = false;
                } else {
                    state.engine = Some(engine);
                }
                self.ready.notify_all();
                Ok(())
            }
            Err(error) => {
                state.error = Some(format!("{error:#}"));
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

        // Parakeet gère lui-même les longues séquences : le moteur découpe
        // seulement au-delà de 25 s, dans les creux de la voix.
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
    }
}

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

impl super::Controller<SystemRecorder, ParakeetTranscriber, SystemPaster> {
    /// Le chef d'orchestre branché sur le vrai micro, le vrai modèle et le vrai
    /// collage — ce que `plume-cli run` et l'app Tauri utilisent.
    pub fn system(settings: super::Settings, events: Sender<super::Event>) -> Self {
        let transcriber = ParakeetTranscriber::new(&settings.model_dir, settings.threads);
        Self::new(
            settings,
            events,
            SystemRecorder::new(),
            transcriber,
            SystemPaster,
        )
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
}
