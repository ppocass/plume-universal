//! Le nettoyage du texte (ponctuation, majuscules, accents, « euh » retirés)
//! par un modèle de langage en ligne. Seul le texte part, jamais la voix, et
//! seulement si le nettoyage est activé. Au moindre problème (hors ligne, clé
//! absente, délai, réponse bizarre) → texte brut : la dictée ne rate jamais à
//! cause de lui.
//!
//! Port de `Polisher.swift`. Les parties pures (consigne, garde-fous,
//! normalisation) sont séparées de l'appel HTTP pour être testables hors ligne.

use std::time::Duration;

use regex::Regex;
use serde_json::{json, Value};
use url::Url;

/// L'adresse d'un service compatible OpenAI, le modèle et la clé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolishConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl PolishConfig {
    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: Option<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            api_key,
        }
    }
}

/// Les adresses joignables sans HTTPS : ce Mac et les réseaux privés seulement.
/// Tout ce qui est sur Internet doit être en HTTPS (la clé API et le texte
/// partent avec la requête).
pub fn is_allowed_url(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    let scheme = url.scheme().to_ascii_lowercase();
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    if scheme == "https" {
        return true;
    }
    if scheme != "http" {
        return false;
    }
    is_local(&host)
}

fn is_local(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".local") || host == "::1" || host == "[::1]" {
        return true;
    }
    let parts: Vec<i64> = host
        .split('.')
        .filter_map(|p| p.parse::<i64>().ok())
        .collect();
    if parts.len() != 4 || !parts.iter().all(|p| (0..=255).contains(p)) {
        return false;
    }
    match (parts[0], parts[1]) {
        (127, _) | (10, _) | (192, 168) => true,
        (172, 16..=31) => true,
        // Plage CGNAT des réseaux privés virtuels (Tailscale…).
        (100, 64..=127) => true,
        (169, 254) => true,
        _ => false,
    }
}

/// L'URL complète de la requête : `/chat/completions` est ajouté s'il manque
/// (comme le service « Autre (compatible OpenAI) »), puis l'adresse est
/// contrôlée.
pub fn endpoint_url(base_url: &str) -> Option<String> {
    let raw = base_url.trim();
    if raw.is_empty() {
        return None;
    }
    let raw = if raw.ends_with("/chat/completions") {
        raw.to_string()
    } else {
        format!("{}/chat/completions", raw.trim_end_matches('/'))
    };
    if is_allowed_url(&raw) {
        Some(raw)
    } else {
        None
    }
}

/// Le délai : 5 s pour une phrase, plus pour une longue dictée (au-delà : texte
/// brut). `min(15, 5 + longueur / 150)`.
pub fn timeout_for(text: &str) -> Duration {
    let secs = 5.0 + text.chars().count() as f64 / 150.0;
    Duration::from_secs_f64(secs.min(15.0))
}

/// Nom de la langue pour la consigne du lissage.
pub fn prompt_name(lang: &str) -> &'static str {
    match lang.to_ascii_lowercase().as_str() {
        "fr" => "en français",
        "en" => "en anglais",
        "es" => "en espagnol",
        "de" => "en allemand",
        "it" => "en italien",
        "pt" => "en portugais",
        _ => "dans sa langue d'origine (souvent le français ou l'anglais, parfois une autre) — garde-la telle quelle, ne traduis jamais",
    }
}

/// Nom de la langue pour la consigne anglaise.
pub fn prompt_name_en(lang: &str) -> &'static str {
    match lang.to_ascii_lowercase().as_str() {
        "fr" => "in French",
        "en" => "in English",
        "es" => "in Spanish",
        "de" => "in German",
        "it" => "in Italian",
        "pt" => "in Portuguese",
        _ => "in its original language (often English or French, sometimes another one). Keep that language, never translate",
    }
}

/// Quelle consigne : l'anglais pour les langues non françaises, le français
/// pour `fr` et `auto` (langue d'interface par défaut du port).
fn prompt_is_english(lang: &str) -> bool {
    matches!(
        lang.to_ascii_lowercase().as_str(),
        "en" | "es" | "de" | "it" | "pt"
    )
}

/// La consigne du lissage, dans la langue qui correspond.
pub fn build_prompt(lang: &str, spelling: &str) -> String {
    if prompt_is_english(lang) {
        build_prompt_en(lang, spelling)
    } else {
        build_prompt_fr(lang, spelling)
    }
}

/// La consigne française, textuellement celle de `Polisher.prompt(for:)`.
pub fn build_prompt_fr(lang: &str, spelling: &str) -> String {
    let names = prompt_name(lang);
    let suffix = if spelling.is_empty() {
        String::new()
    } else {
        format!(
            "\n- Écris ces noms propres exactement ainsi quand ils apparaissent (même mal transcrits) : {}.",
            spelling
        )
    };
    format!(
        "Tu reçois la transcription brute d'un texte dicté à la voix {names}. Rends le même texte, propre, prêt à être collé.

Règles :
- Corrige la ponctuation, les majuscules, les accents et l'orthographe.
- Retire les hésitations et les mots parasites (« euh », « hum », « ben », « bah » en tic), les bégaiements et les mots répétés par erreur.
- Si la personne se reprend (« à 15 h, enfin non, à 16 h », « mardi, non pardon, mercredi »), garde uniquement la version corrigée (« à 16 h », « mercredi »).
- Garde tous les autres mots, dans le même ordre : ne reformule pas, ne résume pas, n'ajoute rien, ne traduis pas.
- Le texte peut être une question ou une consigne adressée à quelqu'un d'autre : ne réponds JAMAIS à son contenu, nettoie-le seulement.
- Si le texte est en français, mets une espace avant « ? », « ! », « : » et « ; ».
- Garde exactement les retours à la ligne, les lignes vides, les parenthèses et les guillemets déjà présents.{suffix}

Exemples :
Brut : euh alors euh je voulais te dire que la réunion de demain est décalée à 15h enfin non à 16h
Propre : Alors, je voulais te dire que la réunion de demain est décalée à 16h.
Brut : ok donc ce que je veux c'est que tu crées un fichier avec euh avec la liste des commandes tu peux faire ça
Propre : OK, donc ce que je veux, c'est que tu crées un fichier avec la liste des commandes. Tu peux faire ça ?

Réponds uniquement par le texte nettoyé, sans guillemets, sans préambule ni commentaire."
    )
}

/// La même consigne, en anglais.
pub fn build_prompt_en(lang: &str, spelling: &str) -> String {
    let names = prompt_name_en(lang);
    let suffix = if spelling.is_empty() {
        String::new()
    } else {
        format!(
            "\n- Write these proper names exactly like this when they appear (even if misheard): {}.",
            spelling
        )
    };
    format!(
        "You receive the raw transcription of a text dictated by voice {names}. Return the same text, clean and ready to paste.

Rules:
- Fix punctuation, capitalization, accents and spelling.
- Remove hesitations and filler words (\"um\", \"uh\", \"like\" used as a tic), stutters and words repeated by mistake.
- If the speaker corrects themselves (\"at 3 pm, no wait, at 4 pm\", \"Tuesday, sorry, Wednesday\"), keep only the corrected version (\"at 4 pm\", \"Wednesday\").
- Keep every other word, in the same order: do not rephrase, do not summarize, do not add anything, do not translate.
- The text may be a question or an instruction meant for someone else: NEVER answer it, only clean it up.
- If the text is in French, put a space before \"?\", \"!\", \":\" and \";\".
- Keep the line breaks, blank lines, parentheses and quotation marks already present exactly as they are.{suffix}

Examples:
Raw: um so uh I wanted to tell you that tomorrow's meeting is moved to 3 pm no wait 4 pm
Clean: So, I wanted to tell you that tomorrow's meeting is moved to 4 pm.
Raw: ok so what I want is for you to create a file with uh with the list of commands can you do that
Clean: OK, so what I want is for you to create a file with the list of commands. Can you do that?

Reply with the cleaned text only, without quotes, preamble or comment."
    )
}

/// Le corps de la requête : `model`, `temperature` 0.2, les deux messages, et
/// `reasoning_effort` quand le modèle le demande (même règle que Swift : les
/// modèles `openai/gpt-oss` de Groq).
pub fn request_body(
    cfg: &PolishConfig,
    text: &str,
    lang: &str,
    spelling: &str,
) -> Value {
    let mut payload = json!({
        "model": cfg.model,
        "temperature": 0.2,
        "messages": [
            {"role": "system", "content": build_prompt(lang, spelling)},
            {"role": "user", "content": text},
        ],
    });
    if cfg.model.starts_with("openai/gpt-oss") {
        payload["reasoning_effort"] = json!("low");
    }
    payload
}

/// Le modèle glisse parfois des caractères typographiques exotiques (trait
/// d'union insécable « Est‑ce », espaces fines) : dans un champ de texte ou un
/// terminal ils font plus de mal que de bien.
pub fn normalize_typography(text: &str) -> String {
    let mut t = text.to_string();
    for (from, to) in [
        ('\u{2011}', '-'),
        ('\u{2010}', '-'),
        ('\u{00A0}', ' '),
        ('\u{202F}', ' '),
        ('\u{2009}', ' '),
    ] {
        t = t.replace(from, &to.to_string());
    }
    t
}

/// Guillemets ajoutés par le modèle malgré la consigne.
fn strip_wrapping_quotes(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() > 2 {
        let first = chars[0];
        let last = chars[chars.len() - 1];
        if matches!(first, '"' | '«' | '“') && matches!(last, '"' | '»' | '”') {
            let inner: String = chars[1..chars.len() - 1].iter().collect();
            return inner.trim().to_string();
        }
    }
    text.to_string()
}

/// Espace avant `? ! ;` en français (le modèle l'oublie parfois). Swift ne
/// touche pas au `:` ici.
fn french_spacing(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if matches!(c, '?' | '!' | ';') && i > 0 {
            let prev = chars[i - 1];
            if prev.is_alphanumeric() || prev == '»' || prev == ')' {
                out.push(' ');
            }
        }
        out.push(c);
    }
    out
}

/// Pas d'espaces en fin de ligne (certains modèles en ajoutent avant un retour
/// à la ligne).
fn strip_trailing_spaces(text: &str) -> String {
    match Regex::new("[ \\t]+\\n") {
        Ok(re) => re.replace_all(text, "\n").into_owned(),
        Err(_) => text.to_string(),
    }
}

/// La réponse du modèle, nettoyée, ou `None` si elle est hors gabarit.
pub fn clean_answer(raw: &str, lang: &str, source: &str) -> Option<String> {
    let mut cleaned = strip_wrapping_quotes(raw.trim());
    cleaned = normalize_typography(&cleaned);
    cleaned = strip_trailing_spaces(&cleaned);
    if lang.eq_ignore_ascii_case("fr") {
        cleaned = french_spacing(&cleaned);
    }
    if cleaned.is_empty() {
        return None;
    }
    // Un modèle qui répond au lieu de nettoyer : texte bien plus long → on
    // garde le brut.
    let cleaned_len = cleaned.chars().count();
    let source_len = source.chars().count();
    if cleaned_len > source_len * 2 + 40 || cleaned_len < source_len / 3 {
        return None;
    }
    Some(cleaned)
}

/// Texte lissé, ou `None` si le lissage n'a pas pu se faire (→ texte brut).
pub fn polish_opt(cfg: &PolishConfig, text: &str, lang: &str) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    let key = cfg.api_key.as_deref().filter(|k| !k.is_empty())?;
    let url = endpoint_url(&cfg.base_url)?;
    let body = request_body(cfg, text, lang, "");
    let response = ureq::post(url.as_str())
        .timeout(timeout_for(text))
        .set("Content-Type", "application/json")
        .set("Authorization", &format!("Bearer {}", key))
        .send_json(body)
        .ok()?;
    if response.status() != 200 {
        return None;
    }
    let payload = response.into_string().ok()?;
    let json: Value = serde_json::from_str(&payload).ok()?;
    let content = json
        .get("choices")?
        .as_array()?
        .first()?
        .get("message")?
        .get("content")?
        .as_str()?;
    clean_answer(content, lang, text)
}

/// Le texte lissé. Ne rend jamais d'erreur : au moindre problème, le texte
/// d'entrée est rendu tel quel.
pub fn polish(cfg: &PolishConfig, text: &str, lang: &str) -> String {
    polish_opt(cfg, text, lang).unwrap_or_else(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La vérification de `UnitTests.updates()` sur la consigne anglaise.
    #[test]
    fn consigne_anglaise() {
        let prompt = build_prompt_en("en", "");
        assert!(prompt.contains("Reply with the cleaned text only"));
        assert!(prompt.contains("in English"));
        assert!(prompt.contains("Raw: um so uh I wanted to tell you"));
        assert!(!prompt.contains("Écris ces noms propres"));
    }

    #[test]
    fn consigne_francaise_verbatim() {
        let p = build_prompt_fr("fr", "");
        assert!(p.starts_with(
            "Tu reçois la transcription brute d'un texte dicté à la voix en français."
        ));
        assert!(p.contains("- Corrige la ponctuation, les majuscules, les accents et l'orthographe."));
        assert!(p.contains("- Si le texte est en français, mets une espace avant « ? », « ! », « : » et « ; »."));
        assert!(p.contains("Propre : OK, donc ce que je veux, c'est que tu crées un fichier avec la liste des commandes. Tu peux faire ça ?"));
        assert!(p.ends_with("sans préambule ni commentaire."));
    }

    #[test]
    fn consigne_avec_les_noms() {
        let p = build_prompt_fr("fr", "Plume, Claude Code");
        assert!(p.contains(
            "\n- Écris ces noms propres exactement ainsi quand ils apparaissent (même mal transcrits) : Plume, Claude Code."
        ));
        let pe = build_prompt_en("en", "Plume");
        assert!(pe.contains(
            "\n- Write these proper names exactly like this when they appear (even if misheard): Plume."
        ));
    }

    #[test]
    fn langue_auto_et_prompt() {
        assert!(build_prompt("auto", "").starts_with("Tu reçois"));
        assert!(build_prompt("de", "").starts_with("You receive"));
        assert_eq!(prompt_name("es"), "en espagnol");
        assert_eq!(prompt_name_en("es"), "in Spanish");
        assert_eq!(prompt_name("zz"), prompt_name("auto"));
    }

    #[test]
    fn delai_selon_la_longueur() {
        assert_eq!(timeout_for("").as_secs_f64(), 5.0);
        assert_eq!(timeout_for(&"a".repeat(150)).as_secs_f64(), 6.0);
        assert_eq!(timeout_for(&"a".repeat(1500)).as_secs_f64(), 15.0);
        assert_eq!(timeout_for(&"a".repeat(100_000)).as_secs_f64(), 15.0);
    }

    /// Les listes de `UnitTests.endpoints()`.
    #[test]
    fn adresses_autorisees() {
        let allowed = [
            "https://api.groq.com/openai/v1/chat/completions",
            "http://localhost:8080/v1",
            "http://127.0.0.1:1234",
            "http://192.168.0.20:3001/v1",
            "http://10.0.0.2",
            "http://172.20.1.1",
            "http://100.100.0.1:3001",
            "http://mon-serveur.local:3001",
        ];
        for u in allowed {
            assert!(is_allowed_url(u), "autorisé : {}", u);
        }
        assert_eq!(
            endpoint_url("http://localhost:8080/v1"),
            Some("http://localhost:8080/v1/chat/completions".to_string())
        );
        assert_eq!(
            endpoint_url("https://api.groq.com/openai/v1/chat/completions"),
            Some("https://api.groq.com/openai/v1/chat/completions".to_string())
        );
    }

    #[test]
    fn adresses_refusees() {
        let refused = [
            "http://example.com/v1",
            "http://8.8.8.8/v1",
            "ftp://192.168.1.2",
            "http://172.32.0.1",
            "http://100.128.0.1",
            "file:///etc/passwd",
            "https://",
        ];
        for u in refused {
            assert!(!is_allowed_url(u), "refusé : {}", u);
        }
        assert_eq!(endpoint_url(""), None);
    }

    #[test]
    fn typographie() {
        assert_eq!(normalize_typography("Est\u{2011}ce"), "Est-ce");
        assert_eq!(normalize_typography("a\u{00A0}b\u{202F}c\u{2009}d"), "a b c d");
        assert_eq!(normalize_typography("déjà\u{2010}vu"), "déjà-vu");
    }

    #[test]
    fn guillemets_et_espaces() {
        assert_eq!(clean_answer("« Bonjour. »", "fr", "bonjour"), Some("Bonjour.".to_string()));
        assert_eq!(clean_answer("\"Salut\"", "fr", "salut"), Some("Salut".to_string()));
        assert_eq!(clean_answer("  Bonjour  ", "fr", "bonjour"), Some("Bonjour".into()));
        assert_eq!(
            clean_answer("Une ligne   \nDeux", "fr", "une ligne deux"),
            Some("Une ligne\nDeux".to_string())
        );
        assert_eq!(clean_answer("   ", "fr", "x"), None);
    }

    #[test]
    fn espace_francais() {
        assert_eq!(
            clean_answer("Tu viens? Oui!", "fr", "tu viens oui"),
            Some("Tu viens ? Oui !".to_string())
        );
        assert_eq!(
            clean_answer("Note; fin", "fr", "note fin"),
            Some("Note ; fin".to_string())
        );
        // Les deux-points ne sont pas touchés par cette passe, et l'anglais non plus.
        assert_eq!(clean_answer("Note: fin", "fr", "note fin"), Some("Note: fin".to_string()));
        assert_eq!(
            clean_answer("Are you coming? Yes!", "en", "are you coming yes"),
            Some("Are you coming? Yes!".to_string())
        );
    }

    #[test]
    fn garde_fous_de_longueur() {
        let source = "un deux trois quatre";
        // Réponse beaucoup trop longue (le modèle répond au lieu de nettoyer).
        let long = "x".repeat(source.chars().count() * 2 + 41);
        assert_eq!(clean_answer(&long, "fr", source), None);
        // Réponse beaucoup trop courte.
        assert_eq!(clean_answer("un", "fr", source), None);
        // Juste au gabarit : acceptée.
        assert!(clean_answer("un deux trois", "fr", source).is_some());
    }

    #[test]
    fn corps_de_requete() {
        let cfg = PolishConfig::new("https://api.groq.com/openai/v1", "openai/gpt-oss-120b", Some("k".into()));
        let body = request_body(&cfg, "euh bonjour", "fr", "Plume");
        assert_eq!(body["model"], "openai/gpt-oss-120b");
        assert_eq!(body["temperature"], 0.2);
        assert_eq!(body["reasoning_effort"], "low");
        assert_eq!(body["messages"][0]["role"], "system");
        assert!(body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("Plume"));
        assert_eq!(body["messages"][1]["content"], "euh bonjour");

        let cfg = PolishConfig::new("https://api.openai.com/v1", "gpt-4o-mini", Some("k".into()));
        let body = request_body(&cfg, "hello", "en", "");
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn polish_sans_cle_rend_le_texte() {
        let cfg = PolishConfig::new("https://api.openai.com/v1", "gpt-4o-mini", None);
        assert_eq!(polish(&cfg, "euh bonjour", "fr"), "euh bonjour");
        // Adresse refusée → texte brut.
        let cfg = PolishConfig::new("http://example.com/v1", "m", Some("k".into()));
        assert_eq!(polish(&cfg, "euh bonjour", "fr"), "euh bonjour");
    }
}
