//! Moteurs de transcription hors ligne.
//!
//! Deux familles, un seul point d'entrée : [`Engine`] (Parakeet NeMo TDT, ou
//! Whisper). Toutes deux s'appuient sur le crate `sherpa-onnx` 2.0.1, lié ici
//! en mode **partagé**. Les bibliothèques natives se trouvent avec la variable
//! d'environnement **`SHERPA_ONNX_LIB_DIR`** (ex. `/sherpa/lib` sur la machine
//! de build) ; sans elle, le script de build du crate télécharge l'archive
//! précompilée correspondant à la plateforme et s'en sert automatiquement.
pub mod parakeet;
pub mod whisper;

pub use parakeet::Parakeet;
pub use whisper::Whisper;

use std::path::Path;

use anyhow::{bail, Result};

use crate::audio::gate::{self, GateVerdict};

/// Au-delà de cette durée, on découpe la dictée (Parakeet gère le reste).
pub const LONG_AUDIO_SECONDS: f64 = 25.0;
/// Longueur minimale d'un morceau découpé.
pub const MIN_CHUNK_SECONDS: f64 = 3.0;

/// La famille d'un modèle : ce que le dossier contient, et ce que le moteur
/// attend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Parakeet,
    Whisper,
}

impl Family {
    /// Le nom des réglages / du catalogue (`parakeet`, `whisper`).
    pub fn as_str(self) -> &'static str {
        match self {
            Family::Parakeet => "parakeet",
            Family::Whisper => "whisper",
        }
    }

    /// Lit un nom de famille (`parakeet`, `whisper`) ; inconnu → `None`.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "parakeet" => Some(Family::Parakeet),
            "whisper" => Some(Family::Whisper),
            _ => None,
        }
    }

    /// Devine la famille d'après les fichiers du dossier : un `joiner*.onnx`
    /// est un transducer NeMo (Parakeet), un `*-encoder*.onnx` est Whisper.
    pub fn detect(dir: &Path) -> Result<Self> {
        if dir.join("joiner.int8.onnx").is_file() || dir.join("joiner.onnx").is_file() {
            return Ok(Family::Parakeet);
        }
        if whisper::looks_like_whisper(dir) {
            return Ok(Family::Whisper);
        }
        bail!(
            "modèle incomplet : ni Parakeet ni Whisper dans {}",
            dir.display()
        )
    }
}

/// Le moteur chargé, quelle que soit sa famille.
pub enum Engine {
    Parakeet(Parakeet),
    Whisper(Whisper),
}

impl Engine {
    /// Charge le modèle de `family` depuis `model_dir`.
    ///
    /// `language` ne sert qu'à Whisper (`auto` = détection) ; Parakeet détecte
    /// toujours la langue seul.
    pub fn load(
        model_dir: &Path,
        family: Family,
        threads: i32,
        language: &str,
    ) -> Result<Self> {
        match family {
            Family::Parakeet => Ok(Self::Parakeet(Parakeet::load(model_dir, threads)?)),
            Family::Whisper => Ok(Self::Whisper(Whisper::load(
                model_dir, threads, language,
            )?)),
        }
    }

    /// Transcrit des échantillons mono à 16 kHz.
    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        match self {
            Self::Parakeet(engine) => engine.transcribe(samples),
            Self::Whisper(engine) => engine.transcribe(samples),
        }
    }

    /// La famille du moteur chargé.
    pub fn family(&self) -> Family {
        match self {
            Self::Parakeet(_) => Family::Parakeet,
            Self::Whisper(_) => Family::Whisper,
        }
    }
}

/// Ce qu'un moteur sait faire : transcrire des échantillons 16 kHz.
///
/// Le trait garde [`transcribe_dictation`] utilisable avec un [`Engine`] comme
/// avec un [`Parakeet`] ou un [`Whisper`] seul.
pub trait Recognizer {
    fn transcribe(&self, samples: &[f32]) -> Result<String>;
}

impl Recognizer for Parakeet {
    fn transcribe(&self, samples: &[f32]) -> Result<String> {
        Parakeet::transcribe(self, samples)
    }
}

impl Recognizer for Whisper {
    fn transcribe(&self, samples: &[f32]) -> Result<String> {
        Whisper::transcribe(self, samples)
    }
}

impl Recognizer for Engine {
    fn transcribe(&self, samples: &[f32]) -> Result<String> {
        Engine::transcribe(self, samples)
    }
}

/// Transcrit une dictée complète : barrière anti-silence d'abord, puis le
/// moteur sur la seule zone de parole.
///
/// Le moteur gère lui-même les longues séquences, donc la zone de parole part
/// en **un seul appel** tant qu'elle fait 25 s ou moins. Au-delà, elle est
/// coupée avec [`gate::chunks`] (25 s max, 3 s min) et les morceaux sont
/// recollés par une seule espace. Rend `Ok(None)` quand la barrière refuse.
pub fn transcribe_dictation<E: Recognizer + ?Sized>(
    engine: &E,
    samples: &[f32],
) -> Result<Option<String>> {
    let range = match gate::analyse(samples, gate::SAMPLE_RATE) {
        GateVerdict::Speech { range } => range,
        GateVerdict::Rejected { .. } => return Ok(None),
    };

    let seconds = (range.end - range.start) as f64 / gate::SAMPLE_RATE as f64;
    let ranges = if seconds <= LONG_AUDIO_SECONDS {
        vec![range]
    } else {
        gate::chunks(samples, range, LONG_AUDIO_SECONDS, MIN_CHUNK_SECONDS)
    };

    let mut parts = Vec::with_capacity(ranges.len());
    for r in ranges {
        let text = engine.transcribe(&samples[r])?;
        let text = text.trim();
        if !text.is_empty() {
            parts.push(text.to_string());
        }
    }

    let joined = parts.join(" ");
    let joined = joined.trim();
    if joined.is_empty() {
        Ok(None)
    } else {
        Ok(Some(joined.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("plume-engine-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn family_names_round_trip() {
        assert_eq!(Family::Parakeet.as_str(), "parakeet");
        assert_eq!(Family::Whisper.as_str(), "whisper");
        assert_eq!(Family::parse("Whisper"), Some(Family::Whisper));
        assert_eq!(Family::parse(" parakeet "), Some(Family::Parakeet));
        assert_eq!(Family::parse("cloud"), None);
    }

    #[test]
    fn family_detection_reads_the_files() {
        let parakeet = temp_dir("detect-parakeet");
        fs::write(parakeet.join("encoder.int8.onnx"), b"x").unwrap();
        fs::write(parakeet.join("joiner.int8.onnx"), b"x").unwrap();
        assert_eq!(Family::detect(&parakeet).unwrap(), Family::Parakeet);

        let whisper = temp_dir("detect-whisper");
        fs::write(whisper.join("small-encoder.int8.onnx"), b"x").unwrap();
        assert_eq!(Family::detect(&whisper).unwrap(), Family::Whisper);

        let empty = temp_dir("detect-empty");
        assert!(Family::detect(&empty).is_err());

        fs::remove_dir_all(&parakeet).ok();
        fs::remove_dir_all(&whisper).ok();
        fs::remove_dir_all(&empty).ok();
    }

    #[test]
    fn loading_a_missing_model_is_an_error_for_both_families() {
        for family in [Family::Parakeet, Family::Whisper] {
            let error = Engine::load(Path::new("/dossier/absent"), family, 4, "fr")
                .err()
                .expect("un dossier absent ne se charge pas");
            assert!(!format!("{error:#}").is_empty());
        }
    }
}
