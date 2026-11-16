//! La mémoire des noms propres (port de `Vocabulary.swift`).
//!
//! Trois couches, de la plus forte à la plus sûre :
//! 1. la liste est donnée au moteur (`whisper_prompt`) : il écrit directement le
//!    bon mot (Parakeet, lui, n'a pas de prompt) ;
//! 2. les graphies déjà vues sont remplacées mot à mot, sans réseau ;
//! 3. le nettoyage reçoit l'orthographe exacte des noms (`spelling_list`).
//!
//! Le format JSON est celui des types `Codable` Swift : `id`, `word`,
//! `variants`, `addedAt`.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Un mot que le moteur doit bien écrire, et les façons dont il l'a déjà mal
/// entendu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabEntry {
    pub id: String,
    pub word: String,
    #[serde(default)]
    pub variants: Vec<String>,
    #[serde(default)]
    pub added_at: String,
}

impl VocabEntry {
    /// Une fiche neuve, avec un identifiant et une date ISO 8601 comme Swift.
    pub fn new(word: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            word: word.into(),
            variants: Vec::new(),
            added_at: now_iso8601(),
        }
    }

    pub fn with_variants(word: impl Into<String>, variants: Vec<String>) -> Self {
        let mut e = Self::new(word);
        e.variants = variants;
        e
    }
}

/// Copie immuable du vocabulaire pour une dictée.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vocabulary {
    pub entries: Vec<VocabEntry>,
}

impl Vocabulary {
    pub fn new(entries: Vec<VocabEntry>) -> Self {
        Self { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Le « prompt » du moteur : les mots, comme une phrase déjà entendue.
    /// Limité (Whisper ne lit que ~220 jetons de prompt, et une très longue
    /// liste pourrait l'inciter à recaser des mots qui n'ont pas été dits).
    pub fn whisper_prompt(&self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        let mut out = String::new();
        for e in &self.entries {
            let next = if out.is_empty() {
                e.word.clone()
            } else {
                format!("{}, {}", out, e.word)
            };
            if next.chars().count() > 500 {
                break;
            }
            out = next;
        }
        Some(format!("{}.", out))
    }

    /// Pour la consigne du lissage.
    pub fn spelling_list(&self) -> String {
        self.entries
            .iter()
            .map(|e| e.word.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Remplace les graphies mal entendues connues, et remet la bonne casse
    /// (« plume » → « Plume »). Mots entiers seulement, insensible à la
    /// casse ; une espace dans une graphie accepte aussi un tiret.
    pub fn apply(&self, text: &str) -> String {
        let mut t = text.to_string();
        for e in &self.entries {
            for form in e.variants.iter().chain(std::iter::once(&e.word)) {
                if form.trim_matches(|c| c == ' ' || c == '\t').is_empty() {
                    continue;
                }
                // `NSRegularExpression.escapedPattern` puis une espace (échappée
                // ou non) devient `[\s-]+`.
                let escaped = regex::escape(form)
                    .replace("\\ ", "[\\s-]+")
                    .replace(' ', "[\\s-]+");
                let pattern = format!("(?i)(?:{})", escaped);
                let Ok(re) = Regex::new(&pattern) else { continue };
                t = replace_whole_words(&t, &re, &e.word);
            }
        }
        t
    }

    // MARK: Édition

    /// Ajoute un mot, ou une graphie mal entendue à un mot existant.
    pub fn add(&mut self, raw_word: &str, raw_variant: Option<&str>) {
        let word = raw_word.trim();
        if word.is_empty() {
            return;
        }
        let variant = raw_variant.map(str::trim).unwrap_or("");
        if let Some(i) = self
            .entries
            .iter()
            .position(|e| e.word.eq_ignore_ascii_case(word))
        {
            self.entries[i].word = word.to_string();
            if !variant.is_empty()
                && !variant.eq_ignore_ascii_case(word)
                && !self.entries[i]
                    .variants
                    .iter()
                    .any(|v| v.eq_ignore_ascii_case(variant))
            {
                self.entries[i].variants.push(variant.to_string());
            }
        } else {
            let variants = if variant.is_empty() || variant.eq_ignore_ascii_case(word) {
                Vec::new()
            } else {
                vec![variant.to_string()]
            };
            self.entries
                .insert(0, VocabEntry::with_variants(word, variants));
        }
    }

    /// Enregistre un mot modifié dans la fiche (ou un nouveau mot).
    pub fn save(&mut self, entry: VocabEntry) {
        let mut e = entry;
        e.word = e.word.trim().to_string();
        let mut seen: Vec<String> = Vec::new();
        e.variants = e
            .variants
            .iter()
            .map(|v| v.trim().to_string())
            .filter(|v| {
                if v.is_empty() || v.eq_ignore_ascii_case(&e.word) {
                    return false;
                }
                let key = v.to_lowercase();
                if seen.contains(&key) {
                    false
                } else {
                    seen.push(key);
                    true
                }
            })
            .collect();
        if e.word.is_empty() {
            return;
        }
        if let Some(i) = self.entries.iter().position(|x| x.id == e.id) {
            self.entries[i] = e;
        } else if let Some(i) = self
            .entries
            .iter()
            .position(|x| x.word.eq_ignore_ascii_case(&e.word))
        {
            // Le mot existe déjà : on fusionne les formes.
            for v in e.variants {
                if !self.entries[i]
                    .variants
                    .iter()
                    .any(|x| x.eq_ignore_ascii_case(&v))
                {
                    self.entries[i].variants.push(v);
                }
            }
        } else {
            self.entries.insert(0, e);
        }
    }

    pub fn remove_variant(&mut self, variant: &str, id: &str) {
        if let Some(i) = self.entries.iter().position(|e| e.id == id) {
            self.entries[i].variants.retain(|v| v != variant);
        }
    }

    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
    }
}

/// Remplace toutes les occurrences entières (bornes `\p{L}\p{N}` manuelles :
/// la caisse `regex` de Rust n'a pas de look-around).
fn replace_whole_words(text: &str, re: &Regex, replacement: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in re.find_iter(text) {
        let start = m.start();
        let end = m.end();
        let before_ok = text[..start]
            .chars()
            .next_back()
            .map(|c| !c.is_alphanumeric())
            .unwrap_or(true);
        let after_ok = text[end..]
            .chars()
            .next()
            .map(|c| !c.is_alphanumeric())
            .unwrap_or(true);
        if !(before_ok && after_ok) {
            continue;
        }
        out.push_str(&text[last..start]);
        out.push_str(replacement);
        last = end;
    }
    out.push_str(&text[last..]);
    out
}

/// Identifiant au format UUID v4 (assez bon pour une fiche locale).
fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ nanos;
    for byte in c.to_le_bytes() {
        h = (h ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    let hi = h;
    let lo = h.rotate_left(17) ^ c.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (hi >> 32) as u32,
        (hi >> 16) as u16,
        (hi & 0xfff) as u16,
        (((lo >> 48) as u16) & 0x3fff) | 0x8000,
        lo & 0xffff_ffff_ffff
    )
}

/// Date ISO 8601 à la seconde, comme `JSONEncoder.iso` (pas de fraction).
fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, mo, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        mo,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Jours depuis l'époque → (année, mois, jour), algorithme de Howard Hinnant.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les vérifications de `UnitTests.vocabulary()`.
    #[test]
    fn swift_checks() {
        let v = Vocabulary::new(vec![
            VocabEntry::with_variants("Plume", vec!["".into(), "plum".into(), "  ".into()]),
            VocabEntry::with_variants("Claude Code", vec!["cloud code".into()]),
        ]);
        assert_eq!(v.apply("le mot plum ici"), "le mot Plume ici");
        assert_eq!(v.apply("rien à changer"), "rien à changer");
        assert_eq!(v.apply("ouvre cloud-code"), "ouvre Claude Code");
        assert_eq!(v.apply("plumeau"), "plumeau");
    }

    #[test]
    fn bonne_casse_et_mots_entiers() {
        let v = Vocabulary::new(vec![VocabEntry::new("Plume")]);
        assert_eq!(v.apply("plume"), "Plume");
        assert_eq!(v.apply("PLUME"), "Plume");
        assert_eq!(v.apply("les plumes"), "les plumes");
        assert_eq!(v.apply("Plume, plume."), "Plume, Plume.");
    }

    #[test]
    fn graphies_multiples_et_ordre() {
        let v = Vocabulary::new(vec![VocabEntry::with_variants(
            "Claude Code",
            vec!["claude code".into(), "cloud code".into()],
        )]);
        assert_eq!(v.apply("cloud  code"), "Claude Code");
        assert_eq!(v.apply("claude\tcode"), "Claude Code");
        assert_eq!(v.apply("Claude Code"), "Claude Code");
    }

    #[test]
    fn whisper_prompt_et_spelling_list() {
        let v = Vocabulary::new(vec![
            VocabEntry::new("Plume"),
            VocabEntry::new("Claude Code"),
        ]);
        assert_eq!(v.whisper_prompt().as_deref(), Some("Plume, Claude Code."));
        assert_eq!(v.spelling_list(), "Plume, Claude Code");
        assert_eq!(Vocabulary::default().whisper_prompt(), None);
    }

    #[test]
    fn whisper_prompt_limite_a_500() {
        let entries = (0..100)
            .map(|i| VocabEntry::new(format!("mot{:07}", i)))
            .collect();
        let v = Vocabulary::new(entries);
        let p = v.whisper_prompt().unwrap();
        assert!(p.chars().count() <= 501, "prompt trop long : {}", p.len());
        assert!(p.ends_with('.'));
    }

    #[test]
    fn json_comme_swift() {
        let raw = r#"[{"id":"11111111-1111-4111-8111-111111111111","word":"Plume","variants":["plum"],"addedAt":"2026-11-05T14:23:00Z"}]"#;
        let entries: Vec<VocabEntry> = serde_json::from_str(raw).unwrap();
        assert_eq!(entries[0].word, "Plume");
        assert_eq!(entries[0].variants, vec!["plum"]);
        let back = serde_json::to_string(&entries).unwrap();
        assert!(back.contains("\"addedAt\":\"2026-11-05T14:23:00Z\""));
        assert!(back.contains("\"word\":\"Plume\""));
    }

    #[test]
    fn ajout_et_fusion() {
        let mut v = Vocabulary::default();
        v.add("Plume", None);
        v.add("plume", Some("plum"));
        v.add("plume", Some("plum"));
        assert_eq!(v.entries.len(), 1);
        assert_eq!(v.entries[0].word, "plume");
        assert_eq!(v.entries[0].variants, vec!["plum"]);
        v.add("Claude Code", Some("cloud code"));
        assert_eq!(v.entries.len(), 2);
        assert_eq!(v.entries[0].word, "Claude Code");

        let id = v.entries[1].id.clone();
        v.remove_variant("plum", &id);
        assert!(v.entries[1].variants.is_empty());
        v.remove(&id);
        assert_eq!(v.entries.len(), 1);
    }

    #[test]
    fn id_et_date_iso() {
        let e = VocabEntry::new("x");
        assert_eq!(e.id.len(), 36);
        assert_eq!(e.id.chars().filter(|c| *c == '-').count(), 4);
        assert!(e.added_at.len() == 20 && e.added_at.ends_with('Z'));
    }
}
