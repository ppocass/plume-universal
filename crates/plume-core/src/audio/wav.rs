//! Lecture des fichiers WAV : toujours du 16 kHz mono en `f32` à la sortie.
//!
//! Le micro de Plume produit déjà du 16 kHz mono, mais un fichier de test ou
//! une dictée importée peut être dans un autre format : on mixe les canaux et
//! on rééchantillonne linéairement, comme le faisait `SpeechGate.wav` /
//! `AudioRecorder` côté Swift.

use std::path::Path;

use anyhow::{bail, Context, Result};
use hound::{SampleFormat, WavReader};

/// Fréquence de sortie, celle du moteur (Parakeet attend 16 kHz).
pub const TARGET_RATE: u32 = 16_000;

/// Lit `path` et rend des échantillons mono à 16 kHz dans [-1, 1].
pub fn read_wav_16k_mono(path: impl AsRef<Path>) -> Result<Vec<f32>> {
    let path = path.as_ref();
    let mut reader = WavReader::open(path)
        .with_context(|| format!("ouverture du WAV {}", path.display()))?;
    let spec = reader.spec();
    if spec.channels == 0 {
        bail!("WAV sans canal : {}", path.display());
    }

    let read: std::result::Result<Vec<f32>, hound::Error> = match spec.sample_format {
        SampleFormat::Float => {
            if spec.bits_per_sample != 32 {
                bail!(
                    "WAV flottant en {} bits non géré : {}",
                    spec.bits_per_sample,
                    path.display()
                );
            }
            reader.samples::<f32>().collect()
        }
        SampleFormat::Int => match spec.bits_per_sample {
            8 => reader
                .samples::<i8>()
                .map(|s| s.map(|v| v as f32 / 128.0))
                .collect(),
            16 => reader
                .samples::<i16>()
                .map(|s| s.map(|v| v as f32 / 32_768.0))
                .collect(),
            24 => reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / 8_388_608.0))
                .collect(),
            32 => reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / 2_147_483_648.0))
                .collect(),
            bits => bail!(
                "WAV entier en {} bits non géré : {}",
                bits,
                path.display()
            ),
        },
    };
    let interleaved = read.with_context(|| format!("lecture des échantillons de {}", path.display()))?;

    let channels = spec.channels as usize;
    let mono: Vec<f32> = if channels == 1 {
        interleaved
    } else {
        interleaved
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
            .collect()
    };

    if spec.sample_rate == TARGET_RATE {
        Ok(mono)
    } else {
        Ok(resample_linear(&mono, spec.sample_rate, TARGET_RATE))
    }
}

/// Rééchantillonnage linéaire (suffisant pour de la parole).
fn resample_linear(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || samples.is_empty() || from == 0 {
        return samples.to_vec();
    }
    let out_len = (samples.len() as f64 * to as f64 / from as f64).round() as usize;
    let ratio = from as f64 / to as f64;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let index = pos.floor() as usize;
            let frac = (pos - index as f64) as f32;
            let a = samples.get(index).copied().unwrap_or(0.0);
            let b = samples.get(index + 1).copied().unwrap_or(a);
            a + (b - a) * frac
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_resampling_keeps_length_and_level() {
        // 8 kHz -> 16 kHz : deux fois plus d'échantillons, même niveau continu.
        let input = vec![0.5f32; 800];
        let out = resample_linear(&input, 8_000, 16_000);
        assert_eq!(out.len(), 1_600);
        assert!(out.iter().all(|v| (*v - 0.5).abs() < 1e-6));

        // 48 kHz -> 16 kHz : trois fois moins.
        let out = resample_linear(&vec![0.25f32; 4_800], 48_000, 16_000);
        assert_eq!(out.len(), 1_600);
        assert!(out.iter().all(|v| (*v - 0.25).abs() < 1e-6));
    }

    #[test]
    fn reads_a_sixteen_kilo_mono_wav() {
        let path = std::env::temp_dir().join(format!(
            "plume-wav-test-{}.wav",
            std::process::id()
        ));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        {
            let mut writer = hound::WavWriter::create(&path, spec).unwrap();
            for v in [0i16, 16_384, -16_384, 32_767] {
                writer.write_sample(v).unwrap();
            }
            writer.finalize().unwrap();
        }
        let samples = read_wav_16k_mono(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(samples.len(), 4);
        assert!((samples[1] - 0.5).abs() < 1e-4);
        assert!((samples[2] + 0.5).abs() < 1e-4);
    }

    #[test]
    fn downmixes_stereo_and_resamples() {
        let path = std::env::temp_dir().join(format!(
            "plume-wav-stereo-test-{}.wav",
            std::process::id()
        ));
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 32_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        {
            let mut writer = hound::WavWriter::create(&path, spec).unwrap();
            // 1 s de stéréo : gauche à +0,5, droite à -0,5 -> mono à 0.
            for _ in 0..32_000 {
                writer.write_sample(16_384i16).unwrap();
                writer.write_sample(-16_384i16).unwrap();
            }
            writer.finalize().unwrap();
        }
        let samples = read_wav_16k_mono(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(samples.len(), 16_000);
        assert!(samples.iter().all(|v| v.abs() < 1e-4));
    }
}
