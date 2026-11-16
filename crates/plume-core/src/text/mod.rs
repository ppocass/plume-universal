//! Le texte de Plume : vocabulaire (noms propres), commandes vocales, 1337 et
//! lissage. Le lissage et le 1337 sont appelés séparément par l'application.

pub mod leet;
pub mod polish;
pub mod vocabulary;
pub mod voice_commands;

pub use leet::{Level, Leet};
pub use polish::{polish, PolishConfig};
pub use vocabulary::{VocabEntry, Vocabulary};

/// Le texte après transcription : vocabulaire puis commandes vocales, dans
/// l'ordre de `DictationController.process` (le lissage vient après, et le 1337
/// encore après).
pub fn finish(text: &str, vocab: &Vocabulary) -> String {
    let raw = vocab.apply(text);
    voice_commands::apply(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vocabulaire_puis_commandes() {
        let vocab = Vocabulary::new(vec![VocabEntry::with_variants(
            "Plume",
            vec!["plum".into()],
        )]);
        let out = finish("ouvre plum, à la ligne, merci", &vocab);
        assert_eq!(out, "ouvre Plume\nMerci");
    }

    #[test]
    fn sans_vocabulaire() {
        assert_eq!(finish("Bonjour.", &Vocabulary::default()), "Bonjour.");
        assert_eq!(
            finish("Rien à faire ici", &Vocabulary::default()),
            "Rien à faire ici"
        );
    }
}
