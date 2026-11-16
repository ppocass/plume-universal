//! Le moteur Whisper (sherpa-onnx, modèles `whisper-*` int8).
//!
//! Même reconnaisseur hors ligne que Parakeet, avec la configuration Whisper :
//! encodeur + décodeur ONNX et le fichier `tokens.txt`. La langue vient des
//! réglages (`fr`, `en`…) ; `auto` laisse Whisper la détecter lui-même, et la
//! tâche est toujours `transcribe` (jamais de traduction).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineWhisperModelConfig};

/// Un moteur Whisper chargé : le reconnaisseur hors ligne de sherpa-onnx.
pub struct Whisper {
    recognizer: OfflineRecognizer,
}

impl Whisper {
    /// Charge le modèle depuis `model_dir` avec `threads` fils d'exécution.
    ///
    /// `language` : `auto` (détection), `fr`, `en`… — la langue parlée des
    /// réglages. Les fichiers sont trouvés par leur suffixe (`*-encoder.int8.onnx`,
    /// `*-decoder.int8.onnx`, `*-tokens.txt`) : les archives sherpa-onnx les
    /// préfixent par la taille du modèle (`base-`, `small-`…).
    pub fn load(model_dir: &Path, threads: i32, language: &str) -> Result<Self> {
        let encoder = find_model_file(model_dir, &["-encoder.int8.onnx", "-encoder.onnx"])
            .with_context(|| {
                format!(
                    "modèle Whisper incomplet : encodeur introuvable dans {}",
                    model_dir.display()
                )
            })?;
        let decoder = find_model_file(model_dir, &["-decoder.int8.onnx", "-decoder.onnx"])
            .with_context(|| {
                format!(
                    "modèle Whisper incomplet : décodeur introuvable dans {}",
                    model_dir.display()
                )
            })?;
        let tokens = find_model_file(model_dir, &["-tokens.txt", "tokens.txt"]).with_context(|| {
            format!(
                "modèle Whisper incomplet : tokens.txt introuvable dans {}",
                model_dir.display()
            )
        })?;

        let language = language.trim();
        let language = if language.is_empty()
            || language.eq_ignore_ascii_case("auto")
            || language.eq_ignore_ascii_case("automatic")
        {
            None
        } else {
            Some(language.to_ascii_lowercase())
        };

        let mut config = OfflineRecognizerConfig::default();
        config.model_config.whisper = OfflineWhisperModelConfig {
            encoder: Some(encoder.to_string_lossy().into_owned()),
            decoder: Some(decoder.to_string_lossy().into_owned()),
            language,
            task: Some("transcribe".into()),
            tail_paddings: 0,
            enable_token_timestamps: false,
            enable_segment_timestamps: false,
        };
        config.model_config.tokens = Some(tokens.to_string_lossy().into_owned());
        config.model_config.num_threads = threads;
        config.model_config.debug = false;

        let recognizer = OfflineRecognizer::create(&config).with_context(|| {
            format!(
                "chargement du modèle Whisper dans {}",
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
            .context("Whisper n'a pas rendu de résultat")?;
        Ok(result.text.trim().to_string())
    }
}

/// Le premier fichier du dossier dont le nom finit par l'un des suffixes, dans
/// l'ordre de préférence (`int8` d'abord).
fn find_model_file(dir: &Path, suffixes: &[&str]) -> Option<PathBuf> {
    let entries: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    for suffix in suffixes {
        if let Some(path) = entries
            .iter()
            .find(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().ends_with(suffix))
                    .unwrap_or(false)
            })
        {
            return Some(path.clone());
        }
    }
    None
}

/// Le dossier contient-il les fichiers d'un modèle Whisper ?
pub fn looks_like_whisper(dir: &Path) -> bool {
    find_model_file(dir, &["-encoder.int8.onnx"]).is_some()
        || find_model_file(dir, &["-encoder.onnx"]).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Family;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("plume-whisper-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn whisper_files_are_found_by_suffix() {
        let dir = temp_dir("fichiers");
        fs::write(dir.join("base-encoder.int8.onnx"), b"x").unwrap();
        fs::write(dir.join("base-decoder.int8.onnx"), b"x").unwrap();
        fs::write(dir.join("base-tokens.txt"), b"x").unwrap();
        fs::write(dir.join("base-encoder.onnx"), b"x").unwrap();

        let encoder = find_model_file(&dir, &["-encoder.int8.onnx", "-encoder.onnx"]).unwrap();
        assert_eq!(encoder.file_name().unwrap(), "base-encoder.int8.onnx");
        let decoder = find_model_file(&dir, &["-decoder.int8.onnx", "-decoder.onnx"]).unwrap();
        assert_eq!(decoder.file_name().unwrap(), "base-decoder.int8.onnx");
        let tokens = find_model_file(&dir, &["-tokens.txt", "tokens.txt"]).unwrap();
        assert_eq!(tokens.file_name().unwrap(), "base-tokens.txt");
        assert!(looks_like_whisper(&dir));
        assert_eq!(Family::detect(&dir).unwrap(), Family::Whisper);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_parakeet_dir_is_not_whisper() {
        let dir = temp_dir("parakeet");
        fs::write(dir.join("encoder.int8.onnx"), b"x").unwrap();
        fs::write(dir.join("decoder.int8.onnx"), b"x").unwrap();
        fs::write(dir.join("joiner.int8.onnx"), b"x").unwrap();
        fs::write(dir.join("tokens.txt"), b"x").unwrap();
        assert!(!looks_like_whisper(&dir));
        assert_eq!(Family::detect(&dir).unwrap(), Family::Parakeet);
        assert!(Whisper::load(&dir, 4, "fr").is_err());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_whisper_model_is_an_error() {
        let error = Whisper::load(Path::new("/dossier/qui/n/existe/pas"), 4, "fr")
            .err()
            .expect("un dossier absent ne se charge pas");
        let message = format!("{error:#}");
        assert!(message.contains("Whisper"), "{message}");
    }
}
