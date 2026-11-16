//! Commandes vocales, appliquées avant le nettoyage (instantané, sans réseau) :
//! « à la ligne », « nouveau paragraphe », « ouvrez la parenthèse … fermez la
//! parenthèse », « ouvrez les guillemets », « point d'interrogation »… et leurs
//! équivalents anglais.
//!
//! Port de `VoiceCommands.swift`. La caisse `regex` de Rust n'ayant pas de
//! look-around, les bornes de mot (`\p{L}`/`\p{N}`) sont vérifiées à la main,
//! et ce qui précède la commande (espaces, virgule) est absorbé après coup.

use regex::Regex;

#[derive(Clone, Copy, PartialEq, Eq)]
enum After {
    /// Tout (espaces + ponctuation ajoutée par le moteur).
    All,
    /// Une virgule seulement (fermetures).
    Comma,
    /// Un point ou une virgule (ponctuation).
    Dot,
}

struct Rule {
    pattern: &'static str,
    replacement: &'static str,
    /// Ce qui précède la commande (espaces, virgule) est absorbé ; un point de
    /// fin de phrase reste.
    eat_before: bool,
    eat_after: After,
}

/// Paragraphe avant ligne : « nouveau paragraphe » contient d'autres mots.
const RULES: &[Rule] = &[
    Rule {
        pattern: "nouveau paragraphe|saut de ligne|saute[rz]? une ligne|new paragraph|skip a line",
        replacement: "\n\n",
        eat_before: true,
        eat_after: After::All,
    },
    Rule {
        pattern: "retour [àa] la ligne|[àa] la ligne|nouvelle ligne|new line|newline|next line",
        replacement: "\n",
        eat_before: true,
        eat_after: After::All,
    },
    Rule {
        pattern: "(?:ouvr(?:ez|e|ir|ons)) (?:la |une )?parenth[èe]se|parenth[èe]se ouvrante|open (?:parenthesis|paren|bracket)",
        replacement: " (",
        eat_before: true,
        eat_after: After::All,
    },
    Rule {
        pattern: "(?:ferm(?:ez|e|er|ons)) (?:la )?parenth[èe]se|parenth[èe]se fermante|close (?:parenthesis|paren|bracket)",
        replacement: ")",
        eat_before: true,
        eat_after: After::Comma,
    },
    Rule {
        pattern: "(?:ouvr(?:ez|e|ir|ons)) les guillemets",
        replacement: " « ",
        eat_before: true,
        eat_after: After::All,
    },
    Rule {
        pattern: "(?:ferm(?:ez|e|er|ons)) les guillemets",
        replacement: " »",
        eat_before: true,
        eat_after: After::Comma,
    },
    Rule {
        pattern: "open quote|begin quote",
        replacement: " \"",
        eat_before: true,
        eat_after: After::All,
    },
    Rule {
        pattern: "close quote|end quote|unquote",
        replacement: "\"",
        eat_before: true,
        eat_after: After::Comma,
    },
    Rule {
        pattern: "point d['’]interrogation",
        replacement: " ?",
        eat_before: true,
        eat_after: After::Dot,
    },
    Rule {
        pattern: "point d['’]exclamation",
        replacement: " !",
        eat_before: true,
        eat_after: After::Dot,
    },
    Rule {
        pattern: "deux[- ]points",
        replacement: " :",
        eat_before: true,
        eat_after: After::Dot,
    },
    Rule {
        pattern: "point[- ]virgule",
        replacement: " ;",
        eat_before: true,
        eat_after: After::Dot,
    },
    Rule {
        pattern: "question mark",
        replacement: "?",
        eat_before: true,
        eat_after: After::Dot,
    },
    Rule {
        pattern: "exclamation (?:mark|point)",
        replacement: "!",
        eat_before: true,
        eat_after: After::Dot,
    },
    Rule {
        pattern: "semicolon",
        replacement: ";",
        eat_before: true,
        eat_after: After::Dot,
    },
];

fn after_pattern(after: After) -> &'static str {
    match after {
        After::All => "[ \\t.,;:!?]*",
        After::Comma => ",?",
        After::Dot => "[.,]?",
    }
}

fn build_regex(rule: &Rule) -> Option<Regex> {
    let pattern = format!("(?i)(?:{})(?:{})", rule.pattern, after_pattern(rule.eat_after));
    Regex::new(&pattern).ok()
}

/// Applique toutes les commandes, dans l'ordre des règles.
pub fn apply(text: &str) -> String {
    let mut t = text.to_string();
    for rule in RULES {
        let Some(re) = build_regex(rule) else { continue };
        t = apply_rule(&t, &re, rule.replacement, rule.eat_before);
    }
    if t == text {
        return text.to_string();
    }
    // Mise au propre des espaces autour de ce qui a été inséré.
    tidy(&t)
}

/// Remplace les occurrences de `re` dont la borne gauche (`\p{L}\p{N}`) est
/// respectée, en absorbant `[ \t,]*` avant si demandé.
fn apply_rule(text: &str, re: &Regex, replacement: &str, eat_before: bool) -> String {
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
        if !before_ok {
            continue;
        }
        out.push_str(&text[last..start]);
        if eat_before {
            while out.ends_with(' ') || out.ends_with('\t') || out.ends_with(',') {
                out.pop();
            }
        }
        out.push_str(replacement);
        last = end;
    }
    out.push_str(&text[last..]);
    out
}

fn tidy(text: &str) -> String {
    let mut t = collapse_repeats(text);
    t = replace_re(&t, "[ \\t]+\\n", "\n");
    t = replace_re(&t, "\\n[ \\t]+", "\n");
    t = replace_re(&t, " {2,}", " ");
    t = t.replace("( ", "(");
    t = t.replace(" )", ")");
    t = t.trim_start_matches([' ', '\t']).to_string();
    t = t.trim_end_matches([' ', '\t']).to_string();
    t = space_after_punct_before_letter(&t);
    t = space_after_paren_before_letter(&t);
    capitalize_lines(&t)
}

/// `([?!;:])[ \t]*\1+` → `$1` (les répétitions de ponctuation fusionnent).
fn collapse_repeats(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        out.push(c);
        if matches!(c, '?' | '!' | ';' | ':') {
            let mut j = i + 1;
            loop {
                let mut k = j;
                while k < chars.len() && (chars[k] == ' ' || chars[k] == '\t') {
                    k += 1;
                }
                if k < chars.len() && chars[k] == c {
                    j = k + 1;
                } else {
                    break;
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// `([?!;:])(?=\p{L})` → `$1 `.
fn space_after_punct_before_letter(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        out.push(c);
        if matches!(c, '?' | '!' | ';' | ':')
            && chars.get(i + 1).map(|n| n.is_alphabetic()).unwrap_or(false)
        {
            out.push(' ');
        }
    }
    out
}

/// `\)(?=\p{L})` → `) `.
fn space_after_paren_before_letter(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        out.push(c);
        if c == ')'
            && chars.get(i + 1).map(|n| n.is_alphabetic()).unwrap_or(false)
        {
            out.push(' ');
        }
    }
    out
}

/// Majuscule au début d'une nouvelle ligne.
fn capitalize_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut capitalize_next = false;
    for ch in text.chars() {
        if capitalize_next && ch.is_alphabetic() {
            out.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            out.push(ch);
            if ch == '\n' {
                capitalize_next = true;
            } else if !ch.is_whitespace() {
                capitalize_next = false;
            }
        }
    }
    out
}

fn replace_re(text: &str, pattern: &str, replacement: &str) -> String {
    match Regex::new(pattern) {
        Ok(re) => re.replace_all(text, replacement).into_owned(),
        Err(_) => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les 13 cas de `UnitTests.commands()`.
    #[test]
    fn cas_swift() {
        let cases: [(&str, &str); 13] = [
            ("Bonjour. À la ligne. Comment ça va ?", "Bonjour.\nComment ça va ?"),
            ("Bonjour, à la ligne, comment ça va", "Bonjour\nComment ça va"),
            (
                "Premier point. Nouveau paragraphe. Deuxième point.",
                "Premier point.\n\nDeuxième point.",
            ),
            (
                "Le rendez-vous, ouvrez la parenthèse, mardi, fermez la parenthèse, est confirmé.",
                "Le rendez-vous (mardi) est confirmé.",
            ),
            ("Tu viens demain, point d'interrogation.", "Tu viens demain ?"),
            ("Tu viens demain point d'interrogation", "Tu viens demain ?"),
            (
                "Il a dit ouvrez les guillemets bonjour fermez les guillemets.",
                "Il a dit « bonjour ».",
            ),
            ("First item. New line. Second item.", "First item.\nSecond item."),
            (
                "See the docs, open parenthesis, page two, close parenthesis.",
                "See the docs (page two).",
            ),
            ("Are you coming question mark", "Are you coming?"),
            ("Can you come? Question mark", "Can you come?"),
            ("Rien de spécial ici.", "Rien de spécial ici."),
            ("Une parenthèse dans la phrase.", "Une parenthèse dans la phrase."),
        ];
        for (input, expected) in cases {
            assert_eq!(apply(input), expected, "commande : « {} »", input);
        }
    }

    #[test]
    fn apostrophe_typographique_et_casse() {
        assert_eq!(apply("Vraiment point d’interrogation"), "Vraiment ?");
        assert_eq!(apply("D'abord, À LA LIGNE, la suite"), "D'abord\nLa suite");
        assert_eq!(apply("Deux points ici"), ": ici");
        assert_eq!(apply("Trois points ici"), "Trois points ici");
    }

    #[test]
    fn ponctuation_et_espaces_anglais() {
        assert_eq!(apply("Stop! Exclamation point"), "Stop!");
        assert_eq!(apply("Note semicolon then go"), "Note; then go");
        assert_eq!(apply("Open quote hello close quote"), "\"hello\"");
    }

    #[test]
    fn nettoyage_des_espaces() {
        assert_eq!(apply("Bonjour  ,  à la ligne  .  Suite"), "Bonjour\nSuite");
        assert_eq!(
            apply("Début, ouvrez la parenthèse ,mardi, fermez la parenthèse."),
            "Début (mardi)."
        );
    }

    #[test]
    fn texte_sans_commande_intact() {
        let t = "Un texte normal, avec de la ponctuation : et rien d'autre.";
        assert_eq!(apply(t), t);
    }

    #[test]
    fn bornes_de_mots() {
        // « bravo » contient « à la ligne » ? non, mais « nouvelle ligne » non plus.
        assert_eq!(apply("renouvelle ligne"), "renouvelle ligne");
        assert_eq!(apply("mala ligne"), "mala ligne");
    }
}
