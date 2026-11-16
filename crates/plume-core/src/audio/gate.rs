//! La barrière anti-hallucination et le découpage, portés de `SpeechGate.swift`.
//!
//! 1. **La barrière anti-hallucination** : un détecteur d'activité vocale
//!    simple (niveau, bruit de fond, durée de parole). Un enregistrement sans
//!    vraie parole n'est JAMAIS envoyé : sur du silence, le moteur invente des
//!    phrases.
//! 2. **Le découpage** : une longue dictée est coupée en morceaux de
//!    `max_s` secondes au plus, dans un creux de la voix (fenêtre de 0,25 s la
//!    plus calme), donc entre deux mots plutôt qu'au milieu d'un mot.

use std::ops::Range;

/// Fréquence d'échantillonnage de travail de Plume (comme `AudioRecorder.sampleRate`).
pub const SAMPLE_RATE: u32 = 16_000;

// Seuils mesurés sur des micros de Mac (silence, pièce, clic, voix).
/// Taille d'une trame d'analyse (échantillons).
pub const FRAME_SAMPLES: usize = 4096;
/// En dessous de ce niveau de crête, c'est du silence.
pub const SILENCE_RMS: f32 = 0.008;
/// Plancher du seuil de démarrage de parole.
pub const START_FLOOR: f32 = 0.012;
/// Plancher du seuil de continuation de parole.
pub const CONTINUE_FLOOR: f32 = 0.007;
/// Le seuil suit le bruit de fond, avec ce facteur.
pub const NOISE_MULTIPLIER: f32 = 3.0;
/// Le seuil ne dépasse jamais cette fraction de la crête.
pub const PEAK_CAP: f32 = 0.4;
/// Nombre de trames consécutives au-dessus du seuil pour démarrer la parole.
pub const START_FRAMES: usize = 3;
/// Durée minimale de parole réelle.
pub const MIN_SPEECH_SECONDS: f64 = 0.35;
/// Part minimale de parole dans la zone retenue.
pub const MIN_SPEECH_RATIO: f64 = 0.15;
/// Marge de sécurité avant le premier mot.
pub const PRE_ROLL_SECONDS: f64 = 0.35;
/// Marge de sécurité après le dernier mot.
pub const PAD_SECONDS: f64 = 0.25;
/// Longueur maximale d'un morceau pour le découpage historique (Whisper).
pub const MAX_CHUNK_SECONDS: f64 = 8.0;
/// Longueur minimale d'un morceau pour le découpage historique (Whisper).
pub const MIN_CHUNK_SECONDS: f64 = 3.0;

/// Ce qu'on envoie (ou pas) au moteur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateVerdict {
    /// De la vraie parole, dans cette zone d'échantillons.
    Speech { range: Range<usize> },
    /// Rien à transcrire, avec la même raison qu'en Swift.
    Rejected { reason: String },
}

/// Analyse un enregistrement : silence, bruit de pièce, clic ou parole.
pub fn analyse(samples: &[f32], sample_rate: u32) -> GateVerdict {
    let total = samples.len();
    let frame_count = total / FRAME_SAMPLES;
    if frame_count < 1 {
        return rejected("trop court");
    }

    let mut levels = vec![0.0f32; frame_count];
    for (f, level) in levels.iter_mut().enumerate() {
        let base = f * FRAME_SAMPLES;
        let mut sum = 0.0f32;
        for v in &samples[base..base + FRAME_SAMPLES] {
            sum += v * v;
        }
        *level = (sum / FRAME_SAMPLES as f32).sqrt();
    }

    let mut sorted = levels.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let peak = *sorted.last().expect("frame_count >= 1");
    if peak < SILENCE_RMS {
        return rejected("silence");
    }

    let noise = percentile(&sorted, 0.1);
    let ceiling = peak * PEAK_CAP;
    let threshold = START_FLOOR.max((noise * NOISE_MULTIPLIER).min(ceiling));
    let continue_threshold =
        CONTINUE_FLOOR.max((noise * NOISE_MULTIPLIER * 0.6).min(ceiling));

    let mut first: Option<usize> = None;
    let mut streak = 0usize;
    for i in 0..frame_count {
        if levels[i] > threshold {
            streak += 1;
            if streak >= START_FRAMES {
                first = Some(i + 1 - streak);
                break;
            }
        } else {
            streak = 0;
        }
    }
    let Some(first) = first else {
        return rejected("pas de parole");
    };

    let mut last = first;
    for i in first..frame_count {
        if levels[i] > continue_threshold {
            last = i;
        }
    }

    let mut speech_frames = 0usize;
    for i in first..=last {
        if levels[i] > threshold {
            speech_frames += 1;
        }
    }
    let speech_seconds = (speech_frames * FRAME_SAMPLES) as f64 / sample_rate as f64;
    if speech_seconds < MIN_SPEECH_SECONDS {
        return rejected("trop peu de parole");
    }

    let pre_roll = (PRE_ROLL_SECONDS * sample_rate as f64).round() as usize;
    let pad = (PAD_SECONDS * sample_rate as f64).round() as usize;
    let start = (first * FRAME_SAMPLES).saturating_sub(pre_roll);
    let end = ((last + 1) * FRAME_SAMPLES + pad).min(total);
    if end - start < FRAME_SAMPLES {
        return rejected("trop court");
    }

    let ratio = speech_seconds / ((end - start) as f64 / sample_rate as f64);
    if ratio < MIN_SPEECH_RATIO {
        return rejected("surtout du silence");
    }
    GateVerdict::Speech { range: start..end }
}

fn rejected(reason: &str) -> GateVerdict {
    GateVerdict::Rejected {
        reason: reason.to_string(),
    }
}

/// Le percentile `p` d'un tableau déjà trié (index arrondi, borné).
fn percentile(sorted: &[f32], p: f64) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

// MARK: Découpage

/// Coupe `range` en morceaux de `max_s` au plus, chacun terminé dans le
/// passage le plus calme (fenêtre de 0,25 s) entre `min_s` et `max_s` — donc
/// entre deux mots, pas au milieu d'un.
pub fn chunks(samples: &[f32], range: Range<usize>, max_s: f64, min_s: f64) -> Vec<Range<usize>> {
    let max_len = (max_s * SAMPLE_RATE as f64) as usize;
    let min_len = (min_s * SAMPLE_RATE as f64) as usize;
    let mut out = Vec::new();
    let mut start = range.start;
    while range.end - start > max_len {
        let cut = quietest_point(samples, start + min_len, start + max_len);
        out.push(start..cut);
        start = cut;
    }
    out.push(start..range.end);
    out
}

/// Le milieu de la fenêtre de 0,25 s la plus calme entre `from` et `to` :
/// là où on peut couper sans couper un mot. Une vraie pause entre deux mots
/// dure au moins un quart de seconde.
fn quietest_point(samples: &[f32], from: usize, to: usize) -> usize {
    let window = (0.25 * SAMPLE_RATE as f64) as usize;
    let hop = (0.02 * SAMPLE_RATE as f64) as usize;
    let mut best = to;
    let mut best_energy = f32::MAX;
    let mut w = from;
    while w + window <= to.min(samples.len()) {
        let mut sum = 0.0f32;
        for v in &samples[w..w + window] {
            sum += v * v;
        }
        if sum < best_energy {
            best_energy = sum;
            best = w + window / 2;
        }
        w += hop;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bruit blanc déterministe (pas de dépendance aléatoire) dans [-amplitude, amplitude].
    fn noise(n: usize, amplitude: f32, seed: u32) -> Vec<f32> {
        let mut state = seed;
        (0..n)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let u = (state >> 8) as f32 / 16_777_216.0;
                (u * 2.0 - 1.0) * amplitude
            })
            .collect()
    }

    #[test]
    fn zeros_are_silence() {
        let verdict = analyse(&vec![0.0f32; 16_000], SAMPLE_RATE);
        assert_eq!(
            verdict,
            GateVerdict::Rejected {
                reason: "silence".into()
            }
        );
    }

    #[test]
    fn low_white_noise_is_rejected() {
        // RMS ≈ 0,0087 : au-dessus du silence, sous le plancher de parole.
        let samples = noise(16_000, 0.015, 7);
        assert!(matches!(analyse(&samples, SAMPLE_RATE), GateVerdict::Rejected { .. }));
    }

    #[test]
    fn a_short_click_is_rejected() {
        // Un clic de 40 ms à 1 kHz : deux trames au-dessus du seuil, jamais trois.
        let mut samples = vec![0.0f32; 16_000];
        for i in 8_000..8_640 {
            let t = (i - 8_000) as f32 / SAMPLE_RATE as f32;
            samples[i] = (2.0 * std::f32::consts::PI * 1_000.0 * t).sin();
        }
        assert!(matches!(analyse(&samples, SAMPLE_RATE), GateVerdict::Rejected { .. }));
    }

    #[test]
    fn modulated_noise_is_speech() {
        // Bruit modulé en amplitude (3 Hz), 3 s : une voix synthétique.
        let mut samples = noise(3 * SAMPLE_RATE as usize, 0.3, 11);
        for (i, s) in samples.iter_mut().enumerate() {
            let t = i as f32 / SAMPLE_RATE as f32;
            *s *= 0.8 + 0.2 * (2.0 * std::f32::consts::PI * 3.0 * t).sin();
        }
        match analyse(&samples, SAMPLE_RATE) {
            GateVerdict::Speech { range } => {
                assert_eq!(range.start, 0);
                assert_eq!(range.end, samples.len());
            }
            other => panic!("attendu Speech, obtenu {other:?}"),
        }
    }

    #[test]
    fn chunks_cover_the_range_and_never_exceed_max() {
        // 20 s de bruit entrecoupé de creux : de fausses pauses entre deux mots.
        let mut samples = noise(20 * SAMPLE_RATE as usize, 0.1, 3);
        for (i, s) in samples.iter_mut().enumerate() {
            if (i / 8_000) % 2 == 1 {
                *s = 0.0;
            }
        }
        let ranges = chunks(&samples, 0..samples.len(), 8.0, 3.0);
        let max_len = (8.0 * SAMPLE_RATE as f64) as usize;
        assert_eq!(ranges.first().unwrap().start, 0);
        assert_eq!(ranges.last().unwrap().end, samples.len());
        let mut expected = 0;
        for r in &ranges {
            assert_eq!(r.start, expected);
            assert!(r.end > r.start);
            assert!(r.end - r.start <= max_len);
            expected = r.end;
        }
        assert_eq!(expected, samples.len());
        assert!(ranges.len() >= 3, "20 s en morceaux de 8 s : au moins 3");
    }
}
