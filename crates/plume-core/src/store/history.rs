//! L'historique des dictées — portage de `History.swift`.
//!
//! Deux fichiers dans le dossier de données : `historique.json` (les dictées) et
//! `totals.json` (le compteur cumulé), plus le dossier `audio-a-reessayer/` pour
//! l'audio des dictées ratées.
//!
//! **200 dictées au plus** : à la 201ᵉ, la plus ancienne part avec son éventuel
//! audio d'échec. Le total de l'Accueil, lui, vient du compteur à part, qui ne
//! baisse jamais — c'est ce qui permet d'afficher « 1 225 dictées » alors que
//! l'historique s'arrête à 200.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Local, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{iso, paths, to_json_pretty};

/// Une dictée terminée (`Dictation`).
///
/// Les noms de champs JSON sont ceux de l'app macOS, dans le même ordre :
/// `id`, `date`, `text`, `raw`, `polished`, `pasted`, `appName`, `audioSeconds`,
/// `failed`, `audioFile`, `leet`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dictation {
    pub id: Uuid,
    #[serde(with = "crate::store::iso")]
    pub date: DateTime<Utc>,
    /// Le texte final (nettoyé si le nettoyage a marché).
    pub text: String,
    /// Ce que le moteur a entendu, avant nettoyage.
    pub raw: String,
    pub polished: bool,
    /// `true` : collé dans une app ; `false` : pas de champ de texte au bon
    /// moment, gardé ici.
    pub pasted: bool,
    #[serde(default)]
    pub app_name: Option<String>,
    pub audio_seconds: f64,
    /// La transcription a échoué : l'audio est gardé pour « Réessayer ».
    /// Arrivé après coup, d'où le défaut à la relecture des anciens fichiers.
    #[serde(default)]
    pub failed: bool,
    #[serde(default)]
    pub audio_file: Option<String>,
    /// Dictée 1337 : `text` est le texte maquillé, `raw` garde l'original.
    #[serde(default)]
    pub leet: bool,
}

impl Dictation {
    /// Le nombre de mots, comme `split(whereSeparator: \.isWhitespace)`.
    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }
}

/// Le compteur cumulé (`HistoryStore.Totals`), dans `totals.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Totals {
    pub dictations: usize,
    pub words: usize,
}

/// L'historique, dans `historique.json` du dossier de données.
pub struct History {
    folder: PathBuf,
    file: PathBuf,
    totals_file: PathBuf,
    /// Audio des dictées ratées, pour « Réessayer ».
    audio_folder: PathBuf,
    items: Vec<Dictation>,
    totals: Totals,
}

impl History {
    /// `HistoryStore.limit` : 200 dictées au plus.
    pub const LIMIT: usize = 200;

    /// Ouvre l'historique du dossier de données (`PLUME_DATA_DIR` s'il est
    /// posé), en créant le dossier des audios d'échec.
    pub fn open() -> Self {
        Self::open_in(paths::data_dir())
    }

    /// Ouvre l'historique d'un dossier donné (tests, autre profil).
    pub fn open_in(folder: impl Into<PathBuf>) -> Self {
        let folder = folder.into();
        let audio_folder = folder.join(paths::FAILED_AUDIO_FOLDER);
        let _ = paths::ensure_dir(&audio_folder);
        let mut history = Self {
            file: folder.join("historique.json"),
            totals_file: folder.join("totals.json"),
            audio_folder,
            folder,
            items: Vec::new(),
            totals: Totals::default(),
        };
        history.load();
        history
    }

    /// Relit les deux fichiers.
    ///
    /// Un `historique.json` illisible est **mis de côté** (renommé
    /// `historique.illisible-<horodatage>.json`), jamais écrasé à la prochaine
    /// dictée. Un `totals.json` absent ou illisible est recalculé à partir de ce
    /// que l'historique contient déjà.
    pub fn load(&mut self) {
        if let Ok(bytes) = std::fs::read(&self.file) {
            match serde_json::from_slice::<Vec<Dictation>>(&bytes) {
                Ok(items) => self.items = items,
                Err(error) => {
                    // Fichier abîmé : mis de côté plutôt qu'écrasé.
                    let aside = self
                        .file
                        .with_file_name(format!("historique.illisible-{}.json", Utc::now().timestamp()));
                    let _ = std::fs::rename(&self.file, &aside);
                    log::info!(
                        "historique illisible ({error}), mis de côté : {}",
                        aside.file_name().unwrap_or_default().to_string_lossy()
                    );
                    self.items.clear();
                }
            }
        }

        let totals = std::fs::read(&self.totals_file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Totals>(&bytes).ok());
        match totals {
            Some(totals) => self.totals = totals,
            None => {
                // Première fois : le compteur part de ce que l'historique
                // contient déjà (les dictées ratées ne comptent pas).
                let kept = self.items.iter().filter(|item| !item.failed);
                self.totals = Totals {
                    dictations: kept.clone().count(),
                    words: kept.map(Dictation::word_count).sum(),
                };
                let _ = self.save_totals();
            }
        }

        let before = self.items.len();
        self.trim_to_limit();
        self.remove_orphan_audio();
        if self.items.len() != before {
            let _ = self.save();
        }
    }

    /// Écrit l'historique d'un bloc (fichier temporaire puis `rename`).
    pub fn save(&self) -> std::io::Result<()> {
        let bytes = to_json_pretty(&self.items)?;
        paths::write_atomic(&self.file, &bytes)
    }

    /// Les dictées, la plus récente en premier.
    pub fn items(&self) -> &[Dictation] {
        &self.items
    }

    /// Le compteur cumulé, qui ne baisse jamais.
    pub fn totals(&self) -> Totals {
        self.totals
    }

    /// Le dossier des audios d'échec.
    pub fn audio_folder(&self) -> &Path {
        &self.audio_folder
    }

    /// Le dossier de l'historique (là où sont les deux JSON).
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Ajoute une dictée en tête, puis applique la limite et enregistre.
    pub fn add(&mut self, dictation: Dictation) {
        let words = dictation.word_count();
        let failed = dictation.failed;
        self.items.insert(0, dictation);
        // Une dictée ratée ne compte pas ; elle comptera si un « Réessayer »
        // la remplace par une réussite (voir `replace`).
        if !failed {
            self.count(words);
        }
        self.trim_to_limit();
        let _ = self.save();
    }

    /// Remplace une dictée (même `id`) — un « Réessayer » qui réussit.
    pub fn replace(&mut self, dictation: Dictation) {
        let Some(index) = self.items.iter().position(|item| item.id == dictation.id) else {
            return;
        };
        // Une dictée ratée qui réussit au 2ᵉ essai compte à ce moment-là.
        if self.items[index].failed && !dictation.failed {
            self.count(dictation.word_count());
        }
        self.items[index] = dictation;
        let _ = self.save();
    }

    /// Retire une dictée de l'historique. Son éventuel audio d'échec est effacé
    /// avec elle.
    pub fn delete(&mut self, id: Uuid) {
        let Some(index) = self.items.iter().position(|item| item.id == id) else {
            return;
        };
        if let Some(name) = self.items[index].audio_file.clone() {
            self.remove_audio(&name);
        }
        self.items.remove(index);
        let _ = self.save();
    }

    /// Vide l'historique — le compteur cumulé, lui, reste.
    pub fn clear(&mut self) {
        let names: Vec<String> = self.item_audio().collect();
        for name in names {
            self.remove_audio(&name);
        }
        self.items.clear();
        let _ = self.save();
    }

    /// La dernière dictée réussie (« Copier la dernière dictée »).
    pub fn latest(&self) -> Option<&Dictation> {
        self.items
            .iter()
            .find(|item| !item.failed && !item.text.is_empty())
    }

    /// Les mots des dictées réussies depuis `date` (inclus).
    pub fn words_since(&self, date: DateTime<Utc>) -> usize {
        self.items
            .iter()
            .filter(|item| item.date >= date && !item.failed)
            .map(Dictation::word_count)
            .sum()
    }

    /// Le nombre de dictées réussies depuis `date` (inclus).
    pub fn count_since(&self, date: DateTime<Utc>) -> usize {
        self.items
            .iter()
            .filter(|item| item.date >= date && !item.failed)
            .count()
    }

    /// Les mots d'aujourd'hui : depuis minuit **local**, comme
    /// `Calendar.current.startOfDay(for: Date())`.
    pub fn words_today(&self) -> usize {
        self.words_since(start_of_today())
    }

    /// Les mots des 7 derniers jours : `Date().addingTimeInterval(-7 * 86_400)`,
    /// soit 7 × 24 h glissantes, pas 7 jours de calendrier.
    pub fn words_last_7_days(&self) -> usize {
        self.words_since(Utc::now() - Duration::days(7))
    }

    /// Garde les 200 plus récentes ; l'audio des dictées retirées est effacé.
    fn trim_to_limit(&mut self) {
        if self.items.len() <= Self::LIMIT {
            return;
        }
        self.items.sort_by(|a, b| b.date.cmp(&a.date));
        let dropped: Vec<String> = self.items[Self::LIMIT..]
            .iter()
            .filter_map(|item| item.audio_file.clone())
            .collect();
        for name in dropped {
            self.remove_audio(&name);
        }
        self.items.truncate(Self::LIMIT);
    }

    /// Un audio d'échec qu'aucune dictée ne référence plus.
    fn remove_orphan_audio(&self) {
        let used: HashSet<&str> = self
            .items
            .iter()
            .filter_map(|item| item.audio_file.as_deref())
            .collect();
        let Ok(entries) = std::fs::read_dir(&self.audio_folder) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.ends_with(".wav") && !used.contains(name) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// Les noms des audios référencés par l'historique.
    fn item_audio(&self) -> impl Iterator<Item = String> + '_ {
        self.items
            .iter()
            .filter_map(|item| item.audio_file.clone())
    }

    /// L'audio d'une dictée qu'on retire. macOS l'envoie à la Corbeille ; ici on
    /// l'efface, faute de Corbeille commune — c'est la seule différence assumée
    /// avec `History.swift`.
    fn remove_audio(&self, name: &str) {
        if name.is_empty() {
            return;
        }
        let _ = std::fs::remove_file(self.audio_folder.join(name));
    }

    fn count(&mut self, words: usize) {
        self.totals.dictations += 1;
        self.totals.words += words;
        let _ = self.save_totals();
    }

    fn save_totals(&self) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(&self.totals)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        paths::write_atomic(&self.totals_file, &bytes)
    }
}

/// Minuit local (`Calendar.current.startOfDay(for: Date())`).
fn start_of_today() -> DateTime<Utc> {
    let now = Local::now();
    let midnight = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("minuit existe toujours");
    match Local.from_local_datetime(&midnight).earliest() {
        Some(midnight) => midnight.with_timezone(&Utc),
        // Heure d'été : minuit n'existe pas ce jour-là, on retombe sur
        // « maintenant moins le temps écoulé depuis minuit ».
        None => {
            now.with_timezone(&Utc) - Duration::seconds(i64::from(now.time().num_seconds_from_midnight()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testing::Env;
    use chrono::{Duration, Local, TimeZone};

    fn dictation(date: DateTime<Utc>, text: &str) -> Dictation {
        Dictation {
            id: Uuid::new_v4(),
            date,
            text: text.to_string(),
            raw: text.to_string(),
            polished: false,
            pasted: true,
            app_name: None,
            audio_seconds: 2.0,
            failed: false,
            audio_file: None,
            leet: false,
        }
    }

    /// Le faux historique de `UnitTests.history()` : 250 dictées, la plus
    /// ancienne ratée avec son audio, plus un audio orphelin.
    fn faux_historique(folder: &Path, now: DateTime<Utc>) -> Vec<Dictation> {
        let audio = folder.join(paths::FAILED_AUDIO_FOLDER);
        std::fs::create_dir_all(&audio).expect("dossier audio");
        let mut items = Vec::new();
        for index in 0..250 {
            let mut item = dictation(now - Duration::minutes(index as i64), "un deux trois");
            if index == 249 {
                // la plus ancienne : un échec avec son audio
                item.failed = true;
                item.text.clear();
                item.audio_file = Some("ancienne.wav".to_string());
            }
            items.push(item);
        }
        for name in ["ancienne.wav", "orpheline.wav"] {
            std::fs::write(audio.join(name), [0u8]).expect("audio de test");
        }
        std::fs::write(
            folder.join("historique.json"),
            serde_json::to_vec(&items).expect("JSON"),
        )
        .expect("historique de test");
        items
    }

    /// Un faux historique de 250 dictées : il doit être ramené à 200, sans que
    /// le total cumulé ne baisse. (Port de `UnitTests.history()`.)
    #[test]
    fn store_history_250_ramenees_a_200() {
        let mut env = Env::new();
        let dir = env.data_dir();
        let now = Utc::now().with_nanosecond(0).expect("seconde ronde");
        let written = faux_historique(dir.path(), now);
        let audio = dir.path().join(paths::FAILED_AUDIO_FOLDER);

        let history = History::open();
        assert_eq!(history.items().len(), 200, "historique ramené à 200");
        // Les dates sont écrites à la seconde près (ISO 8601).
        assert_eq!(history.items()[0].date, written[0].date, "la plus récente est gardée");
        assert_eq!(history.items()[199].date, written[199].date, "les 200 plus récentes");
        assert_eq!(history.totals().dictations, 249, "total repris de l'ancien historique");
        assert_eq!(history.totals().words, 249 * 3, "mots repris");
        assert!(!audio.join("ancienne.wav").exists(), "audio de la dictée retirée parti");
        assert!(!audio.join("orpheline.wav").exists(), "audio orphelin parti");
    }

    /// Toujours 200 après une nouvelle dictée, et le total continue de monter ;
    /// le tout se relit tel quel après relance.
    #[test]
    fn store_history_ajout_garde_200_et_le_total() {
        let mut env = Env::new();
        let dir = env.data_dir();
        let now = Utc::now().with_nanosecond(0).expect("seconde ronde");
        faux_historique(dir.path(), now);

        let mut history = History::open();
        history.add(dictation(Utc::now(), "quatre cinq"));
        assert_eq!(history.items().len(), 200, "toujours 200 après une nouvelle dictée");
        assert_eq!(history.items()[0].text, "quatre cinq");
        assert_eq!(history.totals().dictations, 250);
        assert_eq!(history.totals().words, 249 * 3 + 2, "le total continue de monter");

        let reopened = History::open();
        assert_eq!(reopened.items().len(), 200);
        assert_eq!(reopened.totals().dictations, 250);
        assert_eq!(reopened.totals().words, 249 * 3 + 2);
    }

    /// Un historique illisible est mis de côté, pas écrasé.
    #[test]
    fn store_history_illisible_mis_de_cote() {
        let mut env = Env::new();
        let dir = env.data_dir();
        std::fs::write(dir.path().join("historique.json"), b"{pas du json")
            .expect("fichier abîmé");
        let history = History::open();
        assert!(history.items().is_empty(), "historique illisible → liste vide");
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .expect("dossier")
            .map(|entry| entry.expect("entrée").file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            names.iter().any(|name| name.starts_with("historique.illisible-")),
            "fichier illisible mis de côté : {names:?}"
        );
        assert!(
            !dir.path().join("historique.json").exists(),
            "le fichier abîmé a bien été déplacé"
        );
    }

    /// Ajouter, retirer une dictée (son audio part avec elle), puis tout vider
    /// sans toucher au compteur cumulé.
    #[test]
    fn store_history_ajout_et_suppression() {
        let mut env = Env::new();
        let dir = env.data_dir();
        let audio = dir.path().join(paths::FAILED_AUDIO_FOLDER);

        let mut history = History::open();
        let mut ratee = dictation(Utc::now(), "");
        ratee.failed = true;
        ratee.audio_file = Some("ratee.wav".to_string());
        std::fs::write(audio.join("ratee.wav"), [0u8]).expect("audio de test");
        history.add(ratee.clone());
        let premiere = dictation(Utc::now(), "bonjour le monde");
        history.add(premiere.clone());
        assert_eq!(history.items().len(), 2);
        assert_eq!(history.items()[0].id, premiere.id, "la plus récente en tête");
        assert_eq!(history.totals().dictations, 1, "la dictée ratée ne compte pas");
        assert_eq!(history.totals().words, 3);
        assert!(audio.join("ratee.wav").exists(), "l'audio d'échec est gardé");

        // Un « Réessayer » qui réussit compte à ce moment-là.
        let mut reussie = ratee.clone();
        reussie.failed = false;
        reussie.text = "un deux".to_string();
        reussie.audio_file = None;
        history.replace(reussie);
        assert_eq!(history.totals().dictations, 2);
        assert_eq!(history.totals().words, 5);

        // Une autre ratée, avec son audio, pour le vidage.
        let mut autre = dictation(Utc::now(), "");
        autre.failed = true;
        autre.audio_file = Some("autre.wav".to_string());
        std::fs::write(audio.join("autre.wav"), [0u8]).expect("audio de test");
        history.add(autre);

        history.delete(premiere.id);
        assert_eq!(history.items().len(), 2);
        assert_eq!(history.totals().dictations, 2, "le cumulé ne baisse pas");

        history.clear();
        assert!(history.items().is_empty());
        assert_eq!(
            history.totals().dictations,
            2,
            "vider l'historique ne remet pas le compteur à zéro"
        );
        assert!(
            !audio.join("autre.wav").exists(),
            "l'audio de la dictée retirée est parti"
        );

        // À l'ouverture suivante, l'audio devenu orphelin est nettoyé lui aussi.
        let reopened = History::open();
        assert!(reopened.items().is_empty(), "le vidage est enregistré");
        assert_eq!(reopened.totals().dictations, 2);
        assert!(
            !audio.join("ratee.wav").exists(),
            "l'audio sans dictée qui le référence est parti"
        );
    }

    /// Les mots d'aujourd'hui (minuit local) et des 7 derniers jours, les
    /// dictées ratées exclues.
    #[test]
    fn store_history_mots_du_jour_et_7_jours() {
        let mut env = Env::new();
        let _dir = env.data_dir();
        let mut history = History::open();

        // Le début de journée local, calculé comme l'app.
        let now = Local::now();
        let midnight = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("minuit");
        let midnight = Local
            .from_local_datetime(&midnight)
            .earliest()
            .map(|date| date.with_timezone(&Utc))
            .unwrap_or_else(|| Utc::now() - Duration::hours(1));

        history.add(dictation(midnight, "un deux trois"));
        history.add(dictation(midnight - Duration::seconds(1), "hier hier"));
        history.add(dictation(Utc::now() - Duration::days(8), "vieux vieux"));
        history.add(dictation(Utc::now() - Duration::days(6), "six jours"));
        let mut ratee = dictation(Utc::now(), "ratée");
        ratee.failed = true;
        history.add(ratee);

        assert_eq!(history.words_today(), 3, "minuit local inclus, la seconde d'avant exclue");
        assert_eq!(
            history.words_last_7_days(),
            3 + 2 + 2,
            "6 jours comptés, 8 jours non, la ratée jamais"
        );
        assert_eq!(history.words_since(midnight), 3);
        assert_eq!(
            history.count_since(Utc::now() - Duration::days(7)),
            3,
            "aujourd'hui, la seconde d'avant et il y a six jours"
        );
    }

    /// Le JSON écrit est celui de l'app macOS : mêmes clés, même date ISO 8601.
    #[test]
    fn store_history_json_comme_swift() {
        let mut env = Env::new();
        let dir = env.data_dir();
        let mut history = History::open();
        let mut item = dictation(iso::parse("2026-11-05T14:23:45Z").expect("date"), "un deux");
        item.app_name = Some("Mail".to_string());
        item.audio_file = Some("essai.wav".to_string());
        item.leet = true;
        history.add(item.clone());

        let bytes = std::fs::read(dir.path().join("historique.json")).expect("historique écrit");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON lisible");
        let entry = &json[0];
        assert_eq!(entry["date"], "2026-11-05T14:23:45Z");
        assert_eq!(entry["appName"], "Mail");
        assert_eq!(entry["audioSeconds"], 2.0);
        assert_eq!(entry["audioFile"], "essai.wav");
        assert_eq!(entry["failed"], false);
        assert_eq!(entry["leet"], true);
        assert_eq!(entry["id"], item.id.to_string());
        assert_eq!(entry["text"], "un deux");
        assert_eq!(entry["raw"], "un deux");
        assert_eq!(entry["polished"], false);
        assert_eq!(entry["pasted"], true);
        assert_eq!(history.latest().map(Dictation::word_count), Some(2));

        // Et une entrée écrite par la version macOS (sans `failed`, `leet` ni
        // `audioFile`) se relit telle quelle.
        let ancien = r#"[{"id":"e621e1f8-c36c-495a-93fc-0c247a3e6e5f","date":"2026-11-05T14:23:45Z","text":"un deux","raw":"un deux","polished":false,"pasted":true,"appName":null,"audioSeconds":2}]"#;
        std::fs::write(dir.path().join("historique.json"), ancien).expect("ancien fichier");
        let relu = History::open();
        assert_eq!(relu.items().len(), 1);
        assert!(!relu.items()[0].failed);
        assert!(!relu.items()[0].leet);
        assert_eq!(relu.items()[0].audio_file, None);
    }

    /// Le dossier des audios d'échec existe dès l'ouverture.
    #[test]
    fn store_history_dossier_audio() {
        let mut env = Env::new();
        let _dir = env.data_dir();
        let history = History::open();
        assert_eq!(
            history.audio_folder(),
            paths::data_dir().join(paths::FAILED_AUDIO_FOLDER)
        );
        assert!(history.audio_folder().is_dir());
    }
}
