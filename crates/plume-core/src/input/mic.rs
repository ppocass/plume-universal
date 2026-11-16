//! Le micro, seulement pendant une dictée — le pendant de `AudioRecorder.swift`.
//!
//! `start()` à l'appui, `stop()` à la fin : rien n'écoute entre deux dictées.
//! Tout est ramené à la volée en **16 kHz mono** (ce que mange le moteur), sans
//! traitement de la voix (ni AEC ni AGC : un AGC peut baisser le gain matériel
//! du micro et le laisser bas après coup).
//!
//! Le niveau (0…1) suit la formule de macOS : RMS du tampon, en dB, projeté sur
//! −55 dB → 0 et −15 dB → 1.
//!
//! `PLUME_FAKE_AUDIO=<fichier.wav>` remplace le micro par ce fichier, comme
//! dans l'app macOS (pour vérifier touche → pastille → transcription sans
//! parler).

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// La fréquence que mangent Whisper et Parakeet (`AudioRecorder.sampleRate`).
pub const SAMPLE_RATE: u32 = 16_000;

/// Ancien contenu de l'environnement, lu à `start()` (comme macOS, lu au
/// lancement de la dictée : les tests peuvent le poser à chaud).
pub const FAKE_AUDIO_ENV: &str = "PLUME_FAKE_AUDIO";

/// −55 dB (pièce calme) → 0 ; −15 dB (voix forte) → 1.
pub fn level_from_rms(rms: f32) -> f32 {
    let db = 20.0 * rms.max(1e-7).log10();
    ((db + 55.0) / 40.0).clamp(0.0, 1.0)
}

/// Ce que le fil audio partage avec la dictée : les échantillons 16 kHz et le
/// dernier niveau.
#[derive(Default)]
struct Sink {
    samples: Mutex<Vec<f32>>,
    /// Le niveau 0…1, en bits de f32 (`AtomicU32` n'a pas de variante f32).
    level: AtomicU32,
}

impl Sink {
    fn push(&self, mono: &[f32], resampler: &mut LinearResampler) {
        if mono.is_empty() {
            return;
        }
        // macOS calcule le niveau sur le tampon brut, avant conversion.
        let sum: f32 = mono.iter().map(|v| v * v).sum();
        let rms = (sum / mono.len() as f32).sqrt();
        self.level
            .store(level_from_rms(rms).to_bits(), Ordering::Relaxed);
        let converted = resampler.push(mono);
        if let Ok(mut samples) = self.samples.lock() {
            samples.extend_from_slice(&converted);
        }
    }

    fn count(&self) -> usize {
        self.samples.lock().map(|s| s.len()).unwrap_or(0)
    }

    fn copy(&self, from: usize, to: usize) -> Vec<f32> {
        let Ok(samples) = self.samples.lock() else {
            return Vec::new();
        };
        let end = to.min(samples.len());
        if from >= end {
            return Vec::new();
        }
        samples[from..end].to_vec()
    }

    fn take(&self) -> Vec<f32> {
        let Ok(mut samples) = self.samples.lock() else {
            return Vec::new();
        };
        std::mem::take(&mut *samples)
    }

    fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    fn clear(&self) {
        if let Ok(mut samples) = self.samples.lock() {
            samples.clear();
        }
        self.level.store(0.0f32.to_bits(), Ordering::Relaxed);
    }
}

/// Rééchantillonneur linéaire. La position fractionnaire est **conservée d'un
/// tampon au suivant** : sans ça, on perd ou duplique un échantillon à chaque
/// tampon et la durée dérive au fil d'une longue dictée.
pub struct LinearResampler {
    /// Échantillons d'entrée par échantillon de sortie.
    step: f64,
    /// Position de la prochaine sortie. 0 = premier échantillon du tampon
    /// courant, −1 = dernier échantillon du tampon précédent (gardé dans
    /// `last` pour l'interpolation à cheval).
    pos: f64,
    /// Dernier échantillon du tampon précédent.
    last: f32,
    started: bool,
}

impl LinearResampler {
    pub fn new(from: u32, to: u32) -> Self {
        Self {
            step: from as f64 / to as f64,
            pos: 0.0,
            last: 0.0,
            started: false,
        }
    }

    pub fn push(&mut self, input: &[f32]) -> Vec<f32> {
        if input.is_empty() {
            return Vec::new();
        }
        // Même fréquence : rien à faire (et pas de décalage d'un échantillon).
        if self.step == 1.0 {
            return input.to_vec();
        }
        let n = input.len();
        if !self.started {
            self.last = input[0];
            self.started = true;
        }
        let last = self.last;
        // `index` −1 = dernier échantillon du tampon précédent : c'est ce qui
        // permet d'interpoler proprement à la limite entre deux tampons.
        let sample = |index: i64| -> f32 {
            if index < 0 {
                last
            } else {
                input[index as usize]
            }
        };

        let mut out = Vec::with_capacity((n as f64 / self.step) as usize + 2);
        loop {
            let index = self.pos.floor() as i64;
            // Il faut l'échantillon suivant pour interpoler : au-delà, on
            // s'arrête et la position sera reprise au tampon suivant.
            if index + 1 > (n - 1) as i64 {
                break;
            }
            let frac = (self.pos - index as f64) as f32;
            let a = sample(index);
            let b = sample(index + 1);
            out.push(a + (b - a) * frac);
            self.pos += self.step;
        }
        // Le tampon suivant commence `n` échantillons plus loin.
        self.pos -= n as f64;
        self.last = input[n - 1];
        out
    }
}

/// Le micro (ou son remplaçant de test).
pub struct Mic {
    device_name: Option<String>,
    stream: Option<cpal::Stream>,
    sink: Arc<Sink>,
    fake: bool,
    running: bool,
}

impl Default for Mic {
    fn default() -> Self {
        Self::new()
    }
}

impl Mic {
    /// Le micro par défaut du système.
    pub fn new() -> Self {
        Self {
            device_name: None,
            stream: None,
            sink: Arc::new(Sink::default()),
            fake: false,
            running: false,
        }
    }

    /// Un micro choisi par son nom ; s'il n'est plus branché (`start()`), on le
    /// dit au lieu de tomber en silence sur celui du système (macOS, lui,
    /// retombe sur celui de macOS).
    pub fn with_device(name: impl Into<String>) -> Self {
        Self {
            device_name: Some(name.into()),
            ..Self::new()
        }
    }

    /// Ouvre le micro — ou lit le fichier de `PLUME_FAKE_AUDIO`.
    pub fn start(&mut self) -> Result<()> {
        self.sink.clear();
        self.fake = false;

        if let Ok(path) = std::env::var(FAKE_AUDIO_ENV) {
            if !path.is_empty() {
                let samples = read_wav(&path)?;
                // Le fichier factice tient lieu d'enregistrement : on le pose
                // tel quel (macOS fait pareil au moment de `finish()`).
                if let Ok(mut sink) = self.sink.samples.lock() {
                    sink.extend_from_slice(&samples);
                }
                self.fake = true;
                self.running = true;
                return Ok(());
            }
        }

        let host = cpal::default_host();
        let device = match &self.device_name {
            Some(name) => find_input_device(&host, name)
                .ok_or_else(|| anyhow!("micro « {name} » introuvable"))?,
            None => host
                .default_input_device()
                .ok_or_else(|| anyhow!("aucun micro disponible"))?,
        };
        let supported = device
            .default_input_config()
            .context("le micro refuse de donner sa configuration")?;
        let config: cpal::StreamConfig = supported.config();
        let channels = config.channels as usize;
        let rate = config.sample_rate.0;

        let sink = self.sink.clone();
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, sink, channels, rate),
            cpal::SampleFormat::F64 => build_stream::<f64>(&device, &config, sink, channels, rate),
            cpal::SampleFormat::I8 => build_stream::<i8>(&device, &config, sink, channels, rate),
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, sink, channels, rate),
            cpal::SampleFormat::I32 => build_stream::<i32>(&device, &config, sink, channels, rate),
            cpal::SampleFormat::I64 => build_stream::<i64>(&device, &config, sink, channels, rate),
            cpal::SampleFormat::U8 => build_stream::<u8>(&device, &config, sink, channels, rate),
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, sink, channels, rate),
            cpal::SampleFormat::U32 => build_stream::<u32>(&device, &config, sink, channels, rate),
            cpal::SampleFormat::U64 => build_stream::<u64>(&device, &config, sink, channels, rate),
            other => return Err(anyhow!("format audio non géré : {other:?}")),
        }?;

        stream.play().context("le micro refuse de démarrer")?;
        self.stream = Some(stream);
        self.running = true;
        log::info!("micro ouvert ({rate} Hz, {channels} canal/canaux)");
        Ok(())
    }

    /// Arrête le micro et rend tout l'enregistrement (16 kHz mono).
    pub fn stop(&mut self) -> Vec<f32> {
        if let Some(stream) = self.stream.take() {
            let _ = stream.pause();
            drop(stream);
        }
        self.running = false;
        self.fake = false;
        self.sink.take()
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Le micro a-t-il été remplacé par `PLUME_FAKE_AUDIO` ?
    pub fn is_fake(&self) -> bool {
        self.fake
    }

    pub fn level(&self) -> f32 {
        self.sink.level()
    }

    pub fn sample_count(&self) -> usize {
        self.sink.count()
    }

    /// Copie d'une partie de l'enregistrement en cours (transcription au fil de
    /// l'eau).
    pub fn copy(&self, from: usize, to: usize) -> Vec<f32> {
        self.sink.copy(from, to)
    }
}

fn find_input_device(host: &cpal::Host, name: &str) -> Option<cpal::Device> {
    host.input_devices()
        .ok()?
        .find(|device| device.name().map(|n| n == name).unwrap_or(false))
}

/// Construit le flux d'entrée pour un format d'échantillon donné, en ramenant
/// tout de suite en mono puis à 16 kHz.
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sink: Arc<Sink>,
    channels: usize,
    rate: u32,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let mut resampler = LinearResampler::new(rate, SAMPLE_RATE);
    let mut mono: Vec<f32> = Vec::with_capacity(2048);
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            mono.clear();
            for frame in data.chunks(channels) {
                let mut sum = 0.0f32;
                for sample in frame {
                    sum += sample.to_sample::<f32>();
                }
                mono.push(sum / frame.len() as f32);
            }
            sink.push(&mono, &mut resampler);
        },
        |error| log::error!("micro : {error}"),
        None,
    )?;
    Ok(stream)
}

/// Les micros d'entrée du système, par nom.
pub fn list_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices.filter_map(|device| device.name().ok()).collect(),
        Err(error) => {
            log::info!("périphériques audio : {error}");
            Vec::new()
        }
    }
}

/// Lit un WAV (n'importe quel format PCM/float que hound sait ouvrir), le met
/// en mono et à 16 kHz.
pub fn read_wav(path: impl AsRef<Path>) -> Result<Vec<f32>> {
    let path = path.as_ref();
    let mut reader =
        hound::WavReader::open(path).with_context(|| format!("WAV illisible : {}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let scale = (1i64 << (spec.bits_per_sample.saturating_sub(1).min(31))) as f32;
    // Le 8 bits d'un WAV est non signé (128 = silence), tous les autres sont
    // signés : hound rend la valeur brute, c'est à nous de la recentrer.
    let offset = if spec.bits_per_sample == 8 { 128.0 } else { 0.0 };
    let mono: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()?
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
            .collect(),
        hound::SampleFormat::Int => reader
            .samples::<i32>()
            .collect::<std::result::Result<Vec<_>, _>>()?
            .chunks(channels)
            .map(|frame| {
                let sum: f32 = frame.iter().map(|v| (*v as f32 - offset) / scale).sum();
                sum / frame.len() as f32
            })
            .collect(),
    };
    if spec.sample_rate == SAMPLE_RATE {
        return Ok(mono);
    }
    let mut resampler = LinearResampler::new(spec.sample_rate, SAMPLE_RATE);
    Ok(resampler.push(&mono))
}

/// Écrit un WAV 16 kHz mono 16 bits (ce que le moteur attend).
pub fn write_wav(path: impl AsRef<Path>, samples: &[f32]) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("WAV impossible à écrire : {}", path.display()))?;
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
        writer.write_sample(value)?;
    }
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_level_follows_the_mac_formula() {
        // Silence : 0.
        assert_eq!(level_from_rms(0.0), 0.0);
        // −55 dB : le bas de l'échelle.
        assert!((level_from_rms(10f32.powf(-55.0 / 20.0)) - 0.0).abs() < 0.01);
        // −35 dB : la moitié.
        assert!((level_from_rms(10f32.powf(-35.0 / 20.0)) - 0.5).abs() < 0.01);
        // −15 dB et au-delà : 1.
        assert_eq!(level_from_rms(10f32.powf(-15.0 / 20.0)), 1.0);
        assert_eq!(level_from_rms(1.0), 1.0);
    }

    #[test]
    fn resampling_48k_to_16k_keeps_the_duration() {
        let mut resampler = LinearResampler::new(48_000, SAMPLE_RATE);
        let input = vec![0.25f32; 48_000];
        let out = resampler.push(&input);
        assert!((out.len() as i64 - 16_000).abs() <= 2, "{}", out.len());
        assert!(out.iter().all(|v| (*v - 0.25).abs() < 1e-6));
    }

    #[test]
    fn resampling_at_the_same_rate_is_a_passthrough() {
        let mut resampler = LinearResampler::new(SAMPLE_RATE, SAMPLE_RATE);
        let input: Vec<f32> = (0..160).map(|i| i as f32 / 160.0).collect();
        let out = resampler.push(&input);
        assert_eq!(out, input);
    }

    #[test]
    fn resampling_does_not_drift_across_buffers() {
        // 3 s de 44,1 kHz, tampon par tampon : on doit retrouver 3 s à 16 kHz.
        let mut resampler = LinearResampler::new(44_100, SAMPLE_RATE);
        let mut total = 0usize;
        for _ in 0..((44_100 * 3) / 441) {
            total += resampler.push(&vec![0.1f32; 441]).len();
        }
        assert!((total as i64 - 48_000).abs() <= 4, "{total}");
    }

    #[test]
    fn a_wav_round_trips_through_the_writer_and_the_reader() {
        let dir = std::env::temp_dir().join("plume-mic-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("round-trip.wav");
        let samples: Vec<f32> = (0..1_600)
            .map(|i| (i as f32 / 1_600.0 * std::f32::consts::TAU).sin() * 0.5)
            .collect();
        write_wav(&path, &samples).unwrap();
        let read = read_wav(&path).unwrap();
        assert_eq!(read.len(), samples.len());
        for (a, b) in read.iter().zip(samples.iter()) {
            assert!((a - b).abs() < 1e-3, "{a} vs {b}");
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_stereo_wav_is_downmixed_to_mono() {
        let dir = std::env::temp_dir().join("plume-mic-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stereo.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..160 {
            writer.write_sample(16_384i16).unwrap();
            writer.write_sample(-16_384i16).unwrap();
        }
        writer.finalize().unwrap();
        let read = read_wav(&path).unwrap();
        assert_eq!(read.len(), 160);
        assert!(read.iter().all(|v| v.abs() < 1e-3), "{:?}", &read[..4]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn the_fake_audio_replaces_the_microphone() {
        let dir = std::env::temp_dir().join("plume-mic-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fake.wav");
        write_wav(&path, &vec![0.2f32; 800]).unwrap();
        std::env::set_var(FAKE_AUDIO_ENV, &path);
        let mut mic = Mic::new();
        mic.start().unwrap();
        assert!(mic.is_fake());
        assert_eq!(mic.sample_count(), 800);
        let samples = mic.stop();
        assert_eq!(samples.len(), 800);
        assert!(!mic.is_fake());
        std::env::remove_var(FAKE_AUDIO_ENV);
        std::fs::remove_file(&path).ok();
    }
}
