//! Les réglages de Plume — portage de `Prefs.swift`.
//!
//! Swift les garde dans les `UserDefaults`, une clé par réglage ; ici tout vit
//! dans un seul `settings.json`, avec un champ par réglage. Les noms de champs
//! sont les clés de Swift en camelCase (`appLanguage`, `leetEnabled`…), si bien
//! qu'une reprise des réglages de l'app macOS n'a qu'à recopier les clés.
//!
//! Un champ absent prend son défaut, un champ inconnu est ignoré, une valeur
//! inconnue retombe sur le défaut de la clé : c'est le comportement des
//! `UserDefaults.…(forKey:) ?? défaut` de Swift, et il évite qu'un seul réglage
//! abîmé fasse perdre tous les autres.

use std::io;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

use super::{iso, paths, to_json_pretty};
use crate::input::gesture::Key;

/// Les durées de veille du moteur proposées par les réglages
/// (`Prefs.idleChoices` de Swift). `-1` : le moteur ne s'arrête jamais.
pub const IDLE_CHOICES: [f64; 7] = [15.0, 35.0, 60.0, 120.0, 300.0, 900.0, -1.0];

/// Le délai de veille par défaut, en secondes (`Prefs.idleSeconds`).
pub const IDLE_DEFAULT: f64 = 35.0;

/// Le modèle par défaut (`ModelCatalog.defaultID` côté macOS).
pub const DEFAULT_MODEL: &str = "parakeet-v3";

/// La touche de dictée par défaut (`Shortcut.default` : ⌥ gauche sur macOS,
/// qui devient AltLeft ici).
pub const DEFAULT_DICTATION_KEY: &str = "AltLeft";

/// Le modificateur 1337 par défaut (macOS : ⌘ + ⌥ ; le portage documente
/// « Super », c'est-à-dire ⌘/Windows, tenu au moment du Stop).
pub const DEFAULT_LEET_KEY: &str = "Super";

/// Lit une valeur texte et retombe sur le défaut si elle est inconnue —
/// l'équivalent des `RawRepresentable(rawValue:) ?? défaut` de Swift.
fn de_or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr + Default,
{
    let raw = String::deserialize(deserializer)?;
    Ok(T::from_str(&raw).unwrap_or_default())
}

/// Langue de l'interface : français ou anglais. Le texte dicté, lui, reste dans
/// la langue où il a été prononcé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppLanguage {
    Fr,
    /// Défaut (`AppLanguage.current`) : l'anglais, le français se choisit dès la
    /// première page de l'assistant.
    #[default]
    En,
}

impl FromStr for AppLanguage {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Ok(match raw {
            "fr" => Self::Fr,
            "en" => Self::En,
            _ => return Err(()),
        })
    }
}

/// Langue parlée (`DictationLanguage`) : guide Whisper et le lissage ; Parakeet
/// détecte toujours la langue seul, et la langue choisie sert alors au nettoyage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DictationLanguage {
    #[default]
    Auto,
    Fr,
    En,
    Es,
    De,
    It,
    Pt,
}

impl FromStr for DictationLanguage {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Ok(match raw {
            "auto" => Self::Auto,
            "fr" => Self::Fr,
            "en" => Self::En,
            "es" => Self::Es,
            "de" => Self::De,
            "it" => Self::It,
            "pt" => Self::Pt,
            _ => return Err(()),
        })
    }
}

/// Les teintes proposées pour la pastille : vives mais douces. Une sélection
/// plutôt qu'un nuancier libre, pour que la pastille reste toujours lisible sur
/// son fond noir (`PillTint` de Swift).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PillTint {
    #[default]
    Cyan,
    Blue,
    Lavender,
    Violet,
    Pink,
    Coral,
    Red,
    Amber,
    Orange,
    Yellow,
    Mint,
    Green,
}

impl FromStr for PillTint {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Ok(match raw {
            "cyan" => Self::Cyan,
            "blue" => Self::Blue,
            "lavender" => Self::Lavender,
            "violet" => Self::Violet,
            "pink" => Self::Pink,
            "coral" => Self::Coral,
            "red" => Self::Red,
            "amber" => Self::Amber,
            "orange" => Self::Orange,
            "yellow" => Self::Yellow,
            "mint" => Self::Mint,
            "green" => Self::Green,
            _ => return Err(()),
        })
    }
}

impl PillTint {
    /// La couleur exacte de `Prefs.swift` (`#rrggbb`), pour que la pastille
    /// garde les mêmes teintes que sur macOS.
    pub fn hex(self) -> &'static str {
        match self {
            Self::Cyan => "#33D6E0",
            Self::Blue => "#4D9FFF",
            Self::Lavender => "#B4A7FF",
            Self::Violet => "#9D6BFF",
            Self::Pink => "#FF77B7",
            Self::Coral => "#FF8A7A",
            Self::Red => "#FF5A5F",
            Self::Amber => "#F5A623",
            Self::Orange => "#FF8C42",
            Self::Yellow => "#F7D04A",
            Self::Mint => "#5FD29A",
            Self::Green => "#8BD957",
        }
    }
}

/// Le look de la pastille de dictée (`PillStyle`).
///
/// macOS avait trois styles : `bottom`, `top` et `notch` (la carte sortie de
/// l'encoche). L'encoche n'existe pas hors macOS : elle est retirée, et `Top`
/// reprend sa place — même chose posée en haut de l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PillStyle {
    /// En bas de l'écran, au-dessus du Dock (l'emplacement historique).
    Bottom,
    /// En haut : la pastille glisse depuis le bord supérieur.
    #[default]
    Top,
}

impl FromStr for PillStyle {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Ok(match raw {
            "bottom" => Self::Bottom,
            "top" => Self::Top,
            _ => return Err(()),
        })
    }
}

impl PillStyle {
    /// Ce style descend du haut de l'écran (`hangsFromTop` de Swift).
    pub fn hangs_from_top(self) -> bool {
        self != Self::Bottom
    }
}

/// Langue de la consigne envoyée au modèle de nettoyage (pas celle du texte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PromptLanguage {
    #[default]
    Auto,
    Fr,
    En,
}

impl FromStr for PromptLanguage {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Ok(match raw {
            "auto" => Self::Auto,
            "fr" => Self::Fr,
            "en" => Self::En,
            _ => return Err(()),
        })
    }
}

/// Clair, sombre, ou comme le système (`AppAppearance`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppAppearance {
    #[default]
    System,
    Light,
    Dark,
}

impl FromStr for AppAppearance {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Ok(match raw {
            "system" => Self::System,
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => return Err(()),
        })
    }
}

/// Le niveau de maquillage 1337 (`Leet.Level`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LeetLevel {
    Light,
    #[default]
    Medium,
    Full,
}

impl FromStr for LeetLevel {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Ok(match raw {
            "light" => Self::Light,
            "medium" => Self::Medium,
            "full" => Self::Full,
            _ => return Err(()),
        })
    }
}

/// Où se fait le nettoyage (`PolishService`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PolishService {
    /// Le serveur perso (`local.json` sur macOS) ; à choisir avec son adresse.
    Server,
    /// Défaut : Groq (macOS préfère le serveur perso quand il est configuré,
    /// mais il n'y en a pas d'office hors macOS).
    #[default]
    Groq,
    OpenAi,
    /// N'importe quel service compatible OpenAI, avec son adresse.
    Custom,
}

impl FromStr for PolishService {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Ok(match raw {
            "server" => Self::Server,
            "groq" => Self::Groq,
            "openai" => Self::OpenAi,
            "custom" => Self::Custom,
            _ => return Err(()),
        })
    }
}

/// La transcription en ligne, modèle « En ligne » (`TranscriptionService`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionService {
    #[default]
    Groq,
    OpenAi,
    Custom,
}

impl FromStr for TranscriptionService {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Ok(match raw {
            "groq" => Self::Groq,
            "openai" => Self::OpenAi,
            "custom" => Self::Custom,
            _ => return Err(()),
        })
    }
}

/// Les réglages de Plume, dans `settings.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    /// Langue de l'interface (`appLanguage`) : `fr` ou `en`.
    #[serde(default, deserialize_with = "de_or_default")]
    pub app_language: AppLanguage,
    /// Clair, sombre ou comme le système (`appAppearance`).
    #[serde(default, deserialize_with = "de_or_default")]
    pub appearance: AppAppearance,
    /// Langue parlée (`language`) : guide Whisper et le lissage.
    #[serde(default, deserialize_with = "de_or_default")]
    pub language: DictationLanguage,

    /// Touche de dictée, par son nom pour `Key::parse` (`shortcut` de Swift
    /// garde un objet binaire ; ici un nom lisible, « AltLeft » par défaut).
    pub dictation_key: String,
    /// Dictée 1337 : le second geste maquille le texte (`leetEnabled`).
    pub leet_enabled: bool,
    /// Le niveau de maquillage (`leetLevel`).
    #[serde(default, deserialize_with = "de_or_default")]
    pub leet_level: LeetLevel,
    /// Le modificateur 1337 tenu au moment du Stop (`leetShortcut` de Swift :
    /// un couple ⌘+⌥ ; ici un seul nom de touche, « Super » par défaut).
    pub leet_key: String,

    /// Nettoyage du texte par un modèle de langage (`polishEnabled`) : activé
    /// par défaut, le nettoyage retombe sur le texte brut sans réseau.
    pub polish_enabled: bool,
    /// Où se fait le nettoyage (`polishService`).
    #[serde(default, deserialize_with = "de_or_default")]
    pub polish_service: PolishService,
    /// L'adresse du service (`polishCustomURL`) : serveur perso ou compatible
    /// OpenAI. Vide = l'adresse connue du service.
    pub polish_custom_url: String,
    /// Le modèle du service (`polishModel_<service>` dans Swift, un par
    /// service) ; vide = celui par défaut du service.
    pub polish_model: String,
    /// La clé d'API du nettoyage. Swift la garde dans le trousseau ; le
    /// portage n'a pas de trousseau commun, elle est donc ici, en clair.
    pub polish_api_key: String,
    /// Langue de la consigne du nettoyage (`polishPrompt`).
    #[serde(default, deserialize_with = "de_or_default")]
    pub polish_prompt: PromptLanguage,

    /// La transcription en ligne, modèle « En ligne » (`sttService`).
    #[serde(default, deserialize_with = "de_or_default")]
    pub stt_service: TranscriptionService,
    /// L'adresse du service de transcription (`sttCustomURL`).
    pub stt_custom_url: String,
    /// Le modèle de transcription (`sttModel_<service>`).
    pub stt_model: String,

    /// « à la ligne », « ouvrez la parenthèse »… transformés en mise en forme
    /// (`voiceCommands`), activés par défaut.
    pub voice_commands: bool,
    /// Le micro choisi, par son nom (`microphoneUID` côté macOS) ;
    /// `None` = celui du système.
    pub mic_device: Option<String>,

    /// Le modèle de transcription choisi (`model`) : parakeet-v3 par défaut.
    pub model: String,
    /// Le moteur reste chargé ce temps après la dernière dictée, puis il est
    /// arrêté et rend sa mémoire (`engineIdleSeconds`) ; `-1` = jamais.
    pub idle_seconds: f64,

    /// Petit son au début et à la fin d'une dictée (`soundsEnabled`).
    pub sounds_enabled: bool,
    /// Icône dans la barre des menus / la zone de notification
    /// (`showInMenuBar`), désactivée par défaut.
    pub show_tray_icon: bool,
    /// Ouvrir Plume à l'ouverture de session (`launchAtLogin`). Sur macOS c'est
    /// l'état de `SMAppService` ; ici un réglage, activé par défaut.
    pub launch_at_login: bool,

    /// Couleur des barres pendant l'écoute (`listenTint`), cyan par défaut.
    #[serde(default, deserialize_with = "de_or_default")]
    pub listen_tint: PillTint,
    /// Couleur des barres pendant le traitement (`thinkTint`), ambre par défaut.
    #[serde(default, deserialize_with = "de_or_default")]
    pub think_tint: PillTint,
    /// Le look de la pastille (`pillStyle`).
    #[serde(default, deserialize_with = "de_or_default")]
    pub pill_style: PillStyle,

    /// L'assistant de premier lancement a été terminé (`onboardingDone`).
    pub onboarding_done: bool,
    /// **Quand** il a été terminé (`onboardingCompletedAt`) : la seconde preuve,
    /// celle qui ne s'efface pas.
    #[serde(default, with = "iso::optional")]
    pub onboarding_completed_at: Option<DateTime<Utc>>,
    /// La dernière version lancée (`lastRunVersion`) : sert aux « nouveautés ».
    /// `None` = jamais lancée.
    pub last_seen_version: Option<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            app_language: AppLanguage::En,
            appearance: AppAppearance::System,
            language: DictationLanguage::Auto,
            dictation_key: DEFAULT_DICTATION_KEY.to_string(),
            leet_enabled: false,
            leet_level: LeetLevel::Medium,
            leet_key: DEFAULT_LEET_KEY.to_string(),
            polish_enabled: true,
            polish_service: PolishService::default(),
            polish_custom_url: String::new(),
            polish_model: String::new(),
            polish_api_key: String::new(),
            polish_prompt: PromptLanguage::Auto,
            stt_service: TranscriptionService::default(),
            stt_custom_url: String::new(),
            stt_model: String::new(),
            voice_commands: true,
            mic_device: None,
            model: DEFAULT_MODEL.to_string(),
            idle_seconds: IDLE_DEFAULT,
            sounds_enabled: false,
            show_tray_icon: false,
            launch_at_login: true,
            listen_tint: PillTint::Cyan,
            think_tint: PillTint::Amber,
            pill_style: PillStyle::Top,
            onboarding_done: false,
            onboarding_completed_at: None,
            last_seen_version: None,
        }
    }
}

impl Prefs {
    /// Lit `settings.json`.
    ///
    /// Fichier absent → les défauts. Fichier illisible → les défauts aussi, mais
    /// **sans l'écraser** : il reste sur le disque pour inspection, comme un
    /// domaine de préférences qu'on ne sait pas lire côté macOS.
    pub fn load() -> Self {
        let path = paths::settings_file();
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Self::default(),
            Err(error) => {
                log::warn!("réglages illisibles ({}), on repart des défauts : {error}", path.display());
                return Self::default();
            }
        };
        match serde_json::from_slice(&bytes) {
            Ok(prefs) => prefs,
            Err(error) => {
                log::warn!("réglages illisibles ({}), on repart des défauts : {error}", path.display());
                Self::default()
            }
        }
    }

    /// Enregistre les réglages d'un bloc : fichier temporaire puis `rename`,
    /// pour qu'un arrêt au milieu ne laisse jamais un fichier à moitié écrit.
    pub fn save(&self) -> io::Result<()> {
        let bytes = to_json_pretty(self)?;
        paths::write_atomic(&paths::settings_file(), &bytes)
    }

    /// L'assistant de premier lancement doit-il encore s'ouvrir ?
    ///
    /// Port de `Prefs.onboardingDone` : **deux preuves** dans le fichier, jamais
    /// une seule — le drapeau `onboardingDone` et la date
    /// `onboardingCompletedAt`. Si le drapeau a disparu (réglages
    /// réinitialisés, autre profil), la date suffit à garder l'assistant fermé ;
    /// si la date manque (installation antérieure), on la repose sans bruit.
    /// Une réparation est enregistrée aussitôt.
    ///
    /// Le lire ne modifie rien d'autre ; c'est `mark_onboarding_done()` qui pose
    /// les deux preuves.
    pub fn onboarding_needed(&mut self) -> bool {
        if self.onboarding_done {
            // Installation antérieure : on complète la date, sans bruit.
            if self.onboarding_completed_at.is_none() {
                self.onboarding_completed_at = Some(Utc::now());
                let _ = self.save();
            }
            return false;
        }
        // Le drapeau manque : la date, elle, est peut-être encore là.
        if self.onboarding_completed_at.is_some() {
            self.onboarding_done = true;
            log::info!("assistant : drapeau réparé depuis la date de fin");
            let _ = self.save();
            return false;
        }
        true
    }

    /// L'assistant a été terminé pour de bon : les deux preuves sont posées.
    pub fn mark_onboarding_done(&mut self) {
        self.onboarding_done = true;
        if self.onboarding_completed_at.is_none() {
            self.onboarding_completed_at = Some(Utc::now());
        }
        let _ = self.save();
    }

    /// Repose le drapeau à `false` sans effacer la date.
    ///
    /// C'est ce que fait le réglage quand on remet `onboardingDone` à faux ;
    /// « Revoir la configuration… », lui, ouvre l'assistant directement côté
    /// Swift (`OnboardingWindowController.show()`), sans toucher au drapeau. La
    /// date reste — elle dit qu'il *a* été fait une fois. Sinon, fermer
    /// l'assistant sans le refaire le rallumerait à chaque démarrage.
    pub fn reopen_onboarding(&mut self) {
        self.onboarding_done = false;
        let _ = self.save();
    }

    /// La touche de dictée, lue par `Key::parse` ; un nom inconnu retombe sur
    /// `AltLeft`, comme le `?? .default` de Swift.
    pub fn dictation_key(&self) -> Key {
        Key::parse(&self.dictation_key).unwrap_or(Key::AltLeft)
    }

    /// Le modificateur 1337, lu par `Key::parse` ; un nom inconnu retombe sur
    /// `Super` (⌘ / touche Windows).
    pub fn leet_key(&self) -> Key {
        Key::parse(&self.leet_key).unwrap_or(Key::MetaLeft)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testing::Env;
    use chrono::Timelike;
    use std::str::FromStr;

    /// Tous les défauts, tels que les fixe l'app macOS.
    #[test]
    fn store_prefs_defauts() {
        let mut env = Env::new();
        let dir = env.data_dir();
        let mut prefs = Prefs::load();
        assert_eq!(prefs, Prefs::default());
        assert_eq!(prefs.app_language, AppLanguage::En);
        assert_eq!(prefs.language, DictationLanguage::Auto);
        assert_eq!(prefs.dictation_key.as_str(), "AltLeft");
        assert_eq!(prefs.dictation_key(), Key::AltLeft);
        assert_eq!(prefs.leet_key(), Key::MetaLeft);
        assert!(!prefs.leet_enabled);
        assert_eq!(prefs.leet_level, LeetLevel::Medium);
        assert_eq!(prefs.pill_style, PillStyle::Top);
        assert!(prefs.pill_style.hangs_from_top());
        assert_eq!(prefs.listen_tint, PillTint::Cyan);
        assert_eq!(prefs.think_tint, PillTint::Amber);
        assert_eq!(prefs.listen_tint.hex(), "#33D6E0");
        assert!(!prefs.sounds_enabled);
        assert!(!prefs.show_tray_icon);
        assert!(prefs.launch_at_login);
        assert_eq!(prefs.model, DEFAULT_MODEL);
        assert_eq!(prefs.idle_seconds, IDLE_DEFAULT);
        assert!(prefs.mic_device.is_none());
        assert!(prefs.polish_enabled);
        assert!(prefs.voice_commands);
        assert!(prefs.onboarding_completed_at.is_none());
        assert!(!prefs.onboarding_done);
        assert!(prefs.last_seen_version.is_none());
        assert!(prefs.onboarding_needed());
        assert!(!dir.path().join("settings.json").exists(), "rien à réparer, rien d'écrit");
    }

    /// Tout ce qui est enregistré se relit tel quel, et les noms de champs sont
    /// ceux de Swift (camelCase).
    #[test]
    fn store_prefs_aller_retour() {
        let mut env = Env::new();
        let dir = env.data_dir();
        let mut prefs = Prefs::default();
        prefs.app_language = AppLanguage::Fr;
        prefs.language = DictationLanguage::Fr;
        prefs.dictation_key = "f9".to_string();
        prefs.leet_enabled = true;
        prefs.leet_level = LeetLevel::Full;
        prefs.leet_key = "ctrl_r".to_string();
        prefs.polish_enabled = false;
        prefs.polish_service = PolishService::Custom;
        prefs.polish_custom_url = "http://localhost:8080/v1".to_string();
        prefs.polish_model = "gpt-oss-120b".to_string();
        prefs.polish_api_key = "sk-test".to_string();
        prefs.polish_prompt = PromptLanguage::Fr;
        prefs.stt_service = TranscriptionService::OpenAi;
        prefs.stt_custom_url = "http://192.168.0.20:3001/v1".to_string();
        prefs.stt_model = "whisper-1".to_string();
        prefs.voice_commands = false;
        prefs.mic_device = Some("Blue Yeti".to_string());
        prefs.model = "whisper-turbo".to_string();
        prefs.idle_seconds = -1.0;
        prefs.sounds_enabled = true;
        prefs.show_tray_icon = true;
        prefs.launch_at_login = false;
        prefs.pill_style = PillStyle::Bottom;
        prefs.listen_tint = PillTint::Mint;
        prefs.think_tint = PillTint::Pink;
        prefs.last_seen_version = Some("1.5".to_string());
        prefs.mark_onboarding_done();
        assert!(prefs.onboarding_completed_at.is_some());
        // Le fichier garde les dates à la seconde (ISO 8601 sans fraction) : on
        // aligne la valeur en mémoire pour comparer ce qui est vraiment relu.
        prefs.onboarding_completed_at = prefs
            .onboarding_completed_at
            .map(|date| date.with_nanosecond(0).expect("seconde ronde"));
        prefs.save().expect("enregistré");

        let relu = Prefs::load();
        assert_eq!(relu, prefs);

        let bytes = std::fs::read(dir.path().join("settings.json")).expect("settings.json écrit");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON lisible");
        assert_eq!(json["appLanguage"], "fr");
        assert_eq!(json["leetEnabled"], true);
        assert_eq!(json["leetLevel"], "full");
        assert_eq!(json["idleSeconds"], -1.0);
        assert_eq!(json["pillStyle"], "bottom");
        assert_eq!(json["listenTint"], "mint");
        assert_eq!(json["micDevice"], "Blue Yeti");
        assert!(json["onboardingDone"].as_bool().unwrap());
        // Aucun temporaire ne traîne à côté du fichier.
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .expect("dossier")
            .map(|entry| entry.expect("entrée").file_name())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
    }

    /// Un champ absent prend son défaut, un champ inconnu est ignoré.
    #[test]
    fn store_prefs_champs_manquants_et_inconnus() {
        let mut env = Env::new();
        let dir = env.data_dir();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"appLanguage":"fr","champQuiNExistePas":42}"#,
        )
        .expect("fichier de test");
        let prefs = Prefs::load();
        assert_eq!(prefs.app_language, AppLanguage::Fr);
        assert_eq!(prefs.language, DictationLanguage::Auto);
        assert_eq!(prefs.model, DEFAULT_MODEL);
        assert_eq!(prefs.idle_seconds, IDLE_DEFAULT);
        assert_eq!(prefs.pill_style, PillStyle::Top);
        assert!(prefs.launch_at_login);
    }

    /// Une valeur inconnue retombe sur le défaut de **sa** clé, sans emporter
    /// les autres réglages.
    #[test]
    fn store_prefs_valeurs_inconnues() {
        let mut env = Env::new();
        let dir = env.data_dir();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"pillStyle":"notch","leetLevel":"extreme","language":"ja","appLanguage":"de","appearance":"sepia","idleSeconds":900}"#,
        )
        .expect("fichier de test");
        let prefs = Prefs::load();
        assert_eq!(prefs.pill_style, PillStyle::Top, "notch n'existe plus → défaut");
        assert_eq!(prefs.leet_level, LeetLevel::Medium);
        assert_eq!(prefs.language, DictationLanguage::Auto);
        assert_eq!(prefs.app_language, AppLanguage::En);
        assert_eq!(prefs.appearance, AppAppearance::System);
        assert_eq!(prefs.idle_seconds, 900.0, "les autres clés sont gardées");
    }

    /// Un fichier illisible ne fait pas perdre le fichier : on repart des
    /// défauts, sans l'écraser.
    #[test]
    fn store_prefs_fichier_illisible() {
        let mut env = Env::new();
        let dir = env.data_dir();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"{pas du json").expect("fichier de test");
        let prefs = Prefs::load();
        assert_eq!(prefs, Prefs::default());
        assert_eq!(std::fs::read(&path).expect("toujours là"), b"{pas du json");
    }

    /// Neuf : ni drapeau ni date → l'assistant doit s'ouvrir.
    #[test]
    fn store_prefs_onboarding_neuf() {
        let mut env = Env::new();
        let _dir = env.data_dir();
        let mut prefs = Prefs::load();
        assert!(prefs.onboarding_completed_at.is_none());
        assert!(!prefs.onboarding_done);
        assert!(prefs.onboarding_needed(), "jamais fait → l'assistant s'ouvre");
    }

    /// Terminé pour de bon : les deux preuves existent et se relisent.
    #[test]
    fn store_prefs_onboarding_marque_les_deux() {
        let mut env = Env::new();
        let _dir = env.data_dir();
        let mut prefs = Prefs::load();
        prefs.mark_onboarding_done();
        assert!(prefs.onboarding_done);
        assert!(prefs.onboarding_completed_at.is_some());

        let mut relu = Prefs::load();
        assert!(relu.onboarding_done, "drapeau relu");
        assert!(relu.onboarding_completed_at.is_some(), "date relue");
        assert!(!relu.onboarding_needed(), "assistant fermé");
    }

    /// Le drapeau disparaît (réinitialisation, autre identifiant) : la DATE
    /// seule doit suffire à garder l'assistant fermé, et le drapeau est réparé.
    #[test]
    fn store_prefs_onboarding_date_seule_repare() {
        let mut env = Env::new();
        let dir = env.data_dir();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"onboardingCompletedAt":"2026-11-03T10:00:00Z"}"#,
        )
        .expect("fichier de test");
        let mut prefs = Prefs::load();
        assert!(!prefs.onboarding_done);
        assert!(prefs.onboarding_completed_at.is_some());
        assert!(!prefs.onboarding_needed(), "date seule → l'assistant ne s'ouvre pas");
        assert!(prefs.onboarding_done, "drapeau réparé");

        let mut relu = Prefs::load();
        assert!(relu.onboarding_done, "la réparation est enregistrée");
        assert!(!relu.onboarding_needed());
    }

    /// L'inverse : la date manque (installation antérieure au correctif) mais
    /// le drapeau est là → l'assistant reste fermé, et la date est reposée.
    #[test]
    fn store_prefs_onboarding_drapeau_seul_repose_la_date() {
        let mut env = Env::new();
        let dir = env.data_dir();
        std::fs::write(dir.path().join("settings.json"), br#"{"onboardingDone":true}"#)
            .expect("fichier de test");
        let mut prefs = Prefs::load();
        assert!(prefs.onboarding_completed_at.is_none());
        assert!(!prefs.onboarding_needed(), "drapeau seul → l'assistant ne s'ouvre pas");
        assert!(prefs.onboarding_completed_at.is_some(), "date reposée");

        let relu = Prefs::load();
        assert!(relu.onboarding_completed_at.is_some(), "date reposée et enregistrée");
    }

    /// « Revoir la configuration… » : Swift ouvre l'assistant directement
    /// (`OnboardingWindowController.show()`) ; mettre le drapeau à `false` garde
    /// la date. Et comme la date suffit à elle seule, fermer l'assistant sans le
    /// refaire ne le rallume pas au démarrage suivant — exactement le bug
    /// corrigé le 03/11/2026.
    #[test]
    fn store_prefs_onboarding_rouvert_garde_la_date() {
        let mut env = Env::new();
        let _dir = env.data_dir();
        let mut prefs = Prefs::load();
        prefs.mark_onboarding_done();
        let date = prefs.onboarding_completed_at;
        assert!(date.is_some(), "date posée");

        prefs.reopen_onboarding();
        assert!(!prefs.onboarding_done);
        assert_eq!(prefs.onboarding_completed_at, date, "la date est conservée");

        // Le fichier garde les dates à la seconde : on compare les horodatages.
        let mut relu = Prefs::load();
        assert_eq!(
            relu.onboarding_completed_at.map(|date| date.timestamp()),
            date.map(|date| date.timestamp()),
            "la date est toujours là"
        );
        assert!(
            !relu.onboarding_needed(),
            "la date seule garde l'assistant fermé au démarrage"
        );
        assert!(relu.onboarding_done, "le drapeau est réparé depuis la date");

        relu.mark_onboarding_done();
        assert_eq!(
            relu.onboarding_completed_at.map(|date| date.timestamp()),
            date.map(|date| date.timestamp()),
            "la date ne bouge plus"
        );
    }

    /// La touche de dictée se lit par `Key::parse`, et un nom inconnu revient au
    /// défaut plutôt que de casser la dictée.
    #[test]
    fn store_prefs_dictation_key() {
        let mut env = Env::new();
        let _dir = env.data_dir();
        let mut prefs = Prefs::load();
        assert_eq!(prefs.dictation_key(), Key::AltLeft);

        prefs.dictation_key = "super_r".to_string();
        assert_eq!(prefs.dictation_key(), Key::MetaRight);
        prefs.dictation_key = "f13".to_string();
        assert_eq!(prefs.dictation_key(), Key::F(13));
        prefs.leet_key = "alt_r".to_string();
        assert_eq!(prefs.leet_key(), Key::AltRight);

        prefs.save().expect("enregistré");
        assert_eq!(Prefs::load().dictation_key(), Key::F(13));

        prefs.dictation_key = "nawak".to_string();
        assert_eq!(prefs.dictation_key(), Key::AltLeft, "nom inconnu → défaut");
    }

    /// Le délai de veille : mêmes choix que Swift, défaut 35 s, `-1` accepté.
    #[test]
    fn store_prefs_idle_choices() {
        let mut env = Env::new();
        let dir = env.data_dir();
        assert_eq!(IDLE_CHOICES, [15.0, 35.0, 60.0, 120.0, 300.0, 900.0, -1.0]);
        assert_eq!(Prefs::default().idle_seconds, 35.0);
        assert_eq!(Prefs::load().idle_seconds, 35.0);

        std::fs::write(dir.path().join("settings.json"), br#"{"idleSeconds":-1}"#)
            .expect("fichier de test");
        assert_eq!(Prefs::load().idle_seconds, -1.0, "-1 = jamais arrêté");
    }

    /// La pastille : les deux positions retenues, la couleur lisible en hex.
    #[test]
    fn store_prefs_pastille() {
        assert!(PillStyle::from_str("top").unwrap().hangs_from_top());
        assert!(!PillStyle::from_str("bottom").unwrap().hangs_from_top());
        assert!(PillTint::from_str("violet").is_ok());
        assert!(PillTint::from_str("rose").is_err());
        assert_eq!(PillTint::default().hex(), "#33D6E0");
    }
}
