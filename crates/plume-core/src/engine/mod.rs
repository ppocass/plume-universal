//! Moteurs de transcription hors ligne.
//!
//! [`parakeet`] s'appuie sur le crate `sherpa-onnx` 2.0.1, lié ici en mode
//! **partagé**. Les bibliothèques natives se trouvent avec la variable
//! d'environnement **`SHERPA_ONNX_LIB_DIR`** (ex. `/sherpa/lib` sur la machine
//! de build) ; sans elle, le script de build du crate télécharge l'archive
//! précompilée correspondant à la plateforme et s'en sert automatiquement.
pub mod parakeet;

pub use parakeet::Parakeet;

use anyhow::Result;

use crate::audio::gate::{self, GateVerdict};

/// Au-delà de cette durée, on découpe la dictée (Parakeet gère le reste).
pub const LONG_AUDIO_SECONDS: f64 = 25.0;
/// Longueur minimale d'un morceau découpé.
pub const MIN_CHUNK_SECONDS: f64 = 3.0;

/// Transcrit une dictée complète : barrière anti-silence d'abord, puis le
/// moteur sur la seule zone de parole.
///
/// Parakeet gère lui-même les longues séquences, donc la zone de parole part
/// en **un seul appel** tant qu'elle fait 25 s ou moins. Au-delà, elle est
/// coupée avec [`gate::chunks`] (25 s max, 3 s min) et les morceaux sont
/// recollés par une seule espace. Rend `Ok(None)` quand la barrière refuse.
pub fn transcribe_dictation(engine: &Parakeet, samples: &[f32]) -> Result<Option<String>> {
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
