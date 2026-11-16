//! Le second geste de dictée : le texte n'est pas rendu propre, il est maquillé
//! en **1337** (leet speak) — `a → 4`, `e → 3`, `i → 1`, `o → 0`, `s → 5`,
//! `t → 7`, plus une casse décorative.
//!
//! Transformation purement mécanique (comme `VoiceCommands`), locale et
//! instantanée : aucun réseau, aucun modèle. Elle s'applique au texte final
//! (après le vocabulaire, les commandes vocales et le lissage), juste avant le
//! collage.
//!
//! Trois règles qui comptent :
//! - le même texte donne toujours le même résultat : la casse décorative sort
//!   d'un générateur pseudo-aléatoire amorcé par une empreinte du texte (pas du
//!   hasard d'exécution), sinon on ne pourrait pas se relire ;
//! - rien n'est touché dans un lien, une adresse e-mail ou un chemin : un token
//!   qui contient `@`, `/`, `\` ou commence par `www.` reste intact ;
//! - les accents et tout caractère non listé passent inchangés (`é è ç à ù`).

use std::collections::HashMap;

/// Les trois niveaux de maquillage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Light,
    Medium,
    Full,
}

impl Level {
    /// Le nom brut, comme `Leet.Level.rawValue` en Swift.
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Light => "light",
            Level::Medium => "medium",
            Level::Full => "full",
        }
    }

    /// Analyse depuis la ligne de commande (`light`, `medium`, `full`), avec
    /// les accents tolérés.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.to_lowercase().as_str() {
            "light" | "léger" | "leger" => Some(Level::Light),
            "medium" | "moyen" => Some(Level::Medium),
            "full" | "complet" => Some(Level::Full),
            _ => None,
        }
    }
}

/// Tables de substitution, construites comme `merging(_:uniquingKeysWith:)` :
/// c'est la **nouvelle** valeur qu'on garde.
fn table(level: Level) -> HashMap<char, &'static str> {
    let mut m: HashMap<char, &'static str> = HashMap::new();
    for (k, v) in [('a', "4"), ('e', "3"), ('i', "1"), ('o', "0"), ('s', "5"), ('t', "7")] {
        m.insert(k, v);
    }
    if matches!(level, Level::Medium | Level::Full) {
        for (k, v) in [('b', "8"), ('g', "9"), ('l', "1"), ('z', "2")] {
            m.insert(k, v);
        }
    }
    if level == Level::Full {
        for (k, v) in [('c', "("), ('k', "|<"), ('s', "$"), ('x', "><")] {
            m.insert(k, v);
        }
    }
    m
}

/// Le texte maquillé. Déterministe : `transform(t, level) == transform(t, level)`.
pub fn transform(text: &str, level: Level) -> String {
    let map = table(level);
    let mut rng = Rng::new(seed(text));
    let mut out = String::with_capacity(text.len() + text.len() / 4);

    let mut token = String::new();
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !token.is_empty() {
                push_token(&mut out, &token, &map, &mut rng);
                token.clear();
            }
            out.push(ch);
        } else {
            token.push(ch);
        }
    }
    if !token.is_empty() {
        push_token(&mut out, &token, &map, &mut rng);
    }
    out
}

/// Un token protégé (lien, adresse, chemin) passe tel quel ; les autres sont
/// maquillés.
fn push_token(
    out: &mut String,
    token: &str,
    map: &HashMap<char, &'static str>,
    rng: &mut Rng,
) {
    if is_protected(token) {
        out.push_str(token);
    } else {
        out.push_str(&disguise(token, map, rng));
    }
}

/// L'espace de noms `Leet` de Swift (`enum Leet` sans cas).
pub struct Leet;

impl Leet {
    /// Le texte maquillé. Déterministe : `transform(t, level) == transform(t, level)`.
    pub fn transform(text: &str, level: Level) -> String {
        transform(text, level)
    }
}

/// Un token qu'on ne touche pas : lien (`https://…`), adresse e-mail
/// (`pierre@exemple.fr`), chemin de fichier (`/usr/local`, `C:\…`), adresse de
/// site sans schéma (`www.exemple.fr`).
fn is_protected(token: &str) -> bool {
    token.contains('@')
        || token.contains('/')
        || token.contains('\\')
        || token.to_lowercase().starts_with("www.")
}

/// Lettre par lettre : substitution, puis casse décorative (une lettre sur
/// deux, tirée par le générateur — donc stable pour un même texte).
fn disguise(token: &str, map: &HashMap<char, &'static str>, rng: &mut Rng) -> String {
    let mut out = String::with_capacity(token.len());
    for ch in token.chars() {
        // Accents et tout caractère non ASCII : inchangés.
        if !ch.is_ascii() || !ch.is_ascii_alphabetic() {
            out.push(ch);
            continue;
        }
        let upper = rng.bit();
        let lower = ch.to_ascii_lowercase();
        if let Some(replacement) = map.get(&lower) {
            if upper {
                out.push_str(&replacement.to_uppercase());
            } else {
                out.push_str(replacement);
            }
        } else if upper {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push(lower);
        }
    }
    out
}

/// Empreinte FNV-1a du texte : la graine de la casse décorative.
fn seed(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        h = (h ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    if h == 0 {
        0x9E37_79B9_7F4A_7C15
    } else {
        h
    }
}

/// xorshift64 : petit, rapide, et surtout reproductible d'une exécution à
/// l'autre (contrairement au hasard de la bibliothèque standard).
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn bit(&mut self) -> bool {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state & 1 == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les vérifications de `UnitTests.leet()`.
    #[test]
    fn substitutions_du_niveau_leger() {
        assert_eq!(transform("a", Level::Light), "4");
        assert_eq!(transform("e", Level::Light), "3");
        assert_eq!(transform("i", Level::Light), "1");
        assert_eq!(transform("o", Level::Light), "0");
        assert_eq!(transform("s", Level::Light), "5");
        assert_eq!(transform("t", Level::Light), "7");
        // Le léger ne touche pas aux autres lettres (casse décorative seulement).
        assert!(matches!(transform("b", Level::Light).as_str(), "B" | "b"));
    }

    #[test]
    fn niveaux_moyen_et_complet() {
        assert_eq!(transform("b", Level::Medium).to_lowercase(), "8");
        assert_eq!(transform("g", Level::Medium).to_lowercase(), "9");
        assert_eq!(transform("z", Level::Medium).to_lowercase(), "2");
        assert_eq!(transform("k", Level::Full).to_lowercase(), "|<");
        assert_eq!(transform("c", Level::Full).to_lowercase(), "(");
        assert_eq!(transform("s", Level::Full).to_lowercase(), "$");
        // Le moyen garde le s simple.
        assert_eq!(transform("s", Level::Medium).to_lowercase(), "5");
    }

    #[test]
    fn determinisme_et_graines() {
        let phrase = "Le renard brun saute par-dessus le chien paresseux.";
        let once = transform(phrase, Level::Medium);
        for _ in 0..10 {
            assert_eq!(transform(phrase, Level::Medium), once);
        }
        assert_ne!(
            transform("le renard", Level::Medium),
            transform("le chien", Level::Medium)
        );
        assert!(once.chars().any(|c| c.is_numeric()), "des chiffres apparaissent");
        assert!(
            once.chars().count() >= phrase.chars().count(),
            "aucun caractère perdu"
        );
    }

    #[test]
    fn liens_emails_chemins_intacts() {
        let url = "https://exemple.fr/un-chemin?x=1";
        let mail = "pierre@exemple.fr";
        let path = "/usr/local/bin/plume";
        let win_path = r"C:\Users\pierre";
        let www = "www.exemple.fr";
        for token in [url, mail, path, win_path, www] {
            assert_eq!(transform(token, Level::Full), token, "intact : {}", token);
        }

        let with_link = "va voir https://exemple.fr/page ici";
        let linked = transform(with_link, Level::Full);
        let tokens: Vec<&str> = linked.split(' ').collect();
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[2], "https://exemple.fr/page");
        assert_ne!(tokens[0], "va");
        assert_ne!(tokens[1], "voir");
        assert_ne!(tokens[3], "ici");
        assert!(linked.contains("exemple.fr/page"));
    }

    #[test]
    fn accents_et_caracteres_hors_table() {
        for ch in ["é", "è", "ê", "ç", "à", "ù", "œ", "—", "!", "?", "«", "»"] {
            assert_eq!(transform(ch, Level::Full), ch, "inchangé : {}", ch);
        }
        assert!(transform("café", Level::Light).ends_with('é'));
        // La ponctuation n'est pas une lettre : elle reste.
        assert_eq!(transform("!", Level::Full), "!");
    }

    #[test]
    fn vide_espaces_et_tokens() {
        assert_eq!(transform("", Level::Full), "");
        assert_eq!(transform("   ", Level::Full), "   ");
        assert_eq!(transform("a b", Level::Light).chars().count(), 3);
        assert_eq!(transform("a\nb", Level::Light).chars().count(), 3);
    }

    #[test]
    fn les_trois_niveaux_different() {
        let phrase = "Le renard brun saute par-dessus le chien paresseux.";
        assert_ne!(
            transform(phrase, Level::Light),
            transform(phrase, Level::Medium)
        );
        assert_ne!(
            transform(phrase, Level::Medium),
            transform(phrase, Level::Full)
        );
    }

    #[test]
    fn noms_de_niveaux() {
        assert_eq!(Level::parse("light"), Some(Level::Light));
        assert_eq!(Level::parse("Léger"), Some(Level::Light));
        assert_eq!(Level::parse("medium"), Some(Level::Medium));
        assert_eq!(Level::parse("full"), Some(Level::Full));
        assert_eq!(Level::parse("autre"), None);
        assert_eq!(Level::Full.as_str(), "full");
    }
}
