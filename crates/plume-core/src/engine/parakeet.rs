//! Le moteur Parakeet (sherpa-onnx, modèle NeMo TDT 0.6B v3 int8).
//!
//! Port de la partie locale de `SpeechEngine.swift`, sans le serveur HTTP :
//! `sherpa-onnx` est appelé directement, en mémoire.

use std::path::Path;

use anyhow::{bail, Context, Result};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};

/// Les quatre fichiers du modèle int8.
const MODEL_FILES: [&str; 4] = [
    "encoder.int8.onnx",
    "decoder.int8.onnx",
    "joiner.int8.onnx",
    "tokens.txt",
];

/// Un moteur Parakeet chargé : le reconnaisseur hors ligne de sherpa-onnx.
pub struct Parakeet {
    recognizer: OfflineRecognizer,
}

impl Parakeet {
    /// Charge le modèle depuis `model_dir` avec `threads` fils d'exécution.
    ///
    /// 4 fils est le bon réglage mesuré ; 8 n'apporte rien.
    pub fn load(model_dir: &Path, threads: i32) -> Result<Self> {
        for name in MODEL_FILES {
            let file = model_dir.join(name);
            if !file.is_file() {
                bail!(
                    "modèle Parakeet incomplet : {} introuvable",
                    file.display()
                );
            }
        }

        let path = |name: &str| model_dir.join(name).to_string_lossy().into_owned();
        let mut config = OfflineRecognizerConfig::default();
        config.model_config.transducer = OfflineTransducerModelConfig {
            encoder: Some(path("encoder.int8.onnx")),
            decoder: Some(path("decoder.int8.onnx")),
            joiner: Some(path("joiner.int8.onnx")),
        };
        config.model_config.tokens = Some(path("tokens.txt"));
        config.model_config.num_threads = threads;
        // Le même type que le serveur Parakeet de Plume (NeMo transducer).
        config.model_config.model_type = Some("nemo_transducer".into());
        config.model_config.debug = false;

        let recognizer = OfflineRecognizer::create(&config).with_context(|| {
            format!(
                "chargement du modèle Parakeet dans {}",
                model_dir.display()
            )
        })?;
        Ok(Self { recognizer })
    }

    /// Transcrit des échantillons mono à 16 kHz.
    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        let stream = self.recognizer.create_stream();
        stream.accept_waveform(16_000, samples);
        self.recognizer.decode(&stream);
        let result = stream
            .get_result()
            .context("Parakeet n'a pas rendu de résultat")?;
        Ok(result.text.trim().to_string())
    }
}
