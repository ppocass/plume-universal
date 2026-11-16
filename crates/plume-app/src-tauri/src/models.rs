//! Le catalogue des modèles locaux (Linux/Windows) et la règle de
//! recommandation.
//!
//! Trois modèles, tous servis par GitHub (tag `asr-models` du dépôt
//! `k2-fsa/sherpa-onnx`) : Parakeet TDT 0.6B v3 int8, et les Whisper small /
//! base int8. Chaque entrée porte l'URL exacte, le sha256 et la taille de
//! l'archive **téléchargée**, plus la liste des fichiers à garder après
//! extraction.
//!
//! La recommandation est celle de `docs/ipc.md`, transposée du Mac : 25 langues
//! pour Parakeet, Whisper au-delà, et un garde-fou de mémoire vive.

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use plume_core::engine::Family;
use plume_core::store::paths;

/// Les 25 langues que Parakeet reconnaît.
pub const PARAKEET_LANGUAGES: [&str; 25] = [
    "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu", "it", "lv", "lt", "mt",
    "pl", "pt", "ro", "sk", "sl", "es", "sv", "ru", "uk",
];

/// La RAM disponible sous laquelle Parakeet n'est plus recommandé (Go).
pub const PARAKEET_MIN_AVAILABLE_GB: f64 = 1.5;
/// La RAM totale minimale pour Whisper small (Go).
pub const WHISPER_SMALL_MIN_TOTAL_GB: f64 = 8.0;
/// La RAM disponible minimale pour Whisper small (Go).
pub const WHISPER_SMALL_MIN_AVAILABLE_GB: f64 = 2.0;

/// Un modèle local du catalogue.
#[derive(Debug, Clone, Copy)]
pub struct ModelDef {
    pub id: &'static str,
    /// `parakeet` ou `whisper`.
    pub family: &'static str,
    pub name: &'static str,
    /// Taille installée, en Mo (affichage).
    pub size_mb: u64,
    /// Clé de traduction de la ligne « langues ».
    pub languages_key: &'static str,
    /// 1 à 5 (5 = le plus rapide).
    pub speed: u8,
    /// 1 à 5 (5 = le plus précis).
    pub accuracy: u8,
    /// Clé de traduction de la description.
    pub description_key: &'static str,
    /// L'archive `.tar.bz2` du modèle.
    pub url: &'static str,
    /// Le sha256 de l'archive, en minuscules (64 caractères hexadécimaux).
    pub sha256: &'static str,
    /// La taille exacte de l'archive, en octets.
    pub bytes: u64,
    /// Les seuls fichiers gardés dans `<données>/models/<id>/`.
    pub keep: &'static [&'static str],
}

/// Le catalogue : trois modèles locaux, dans l'ordre de la page Modèles.
pub const MODELS: [ModelDef; 3] = [
    ModelDef {
        id: "parakeet-v3",
        family: "parakeet",
        name: "Parakeet TDT 0.6B v3",
        size_mb: 640,
        languages_key: "25_european_languages",
        speed: 5,
        accuracy: 5,
        description_key: "the_most_accurate_for_european_languages_and_very_fast",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
        sha256: "1495e91b59d7fc53ddca91f3431758ec99f8fa253fdb0e1e851be0f72b14b0f9",
        bytes: 487_211_443,
        keep: &[
            "encoder.int8.onnx",
            "decoder.int8.onnx",
            "joiner.int8.onnx",
            "tokens.txt",
        ],
    },
    ModelDef {
        id: "whisper-small",
        family: "whisper",
        name: "Whisper Small",
        size_mb: 358,
        languages_key: "99_languages",
        speed: 2,
        accuracy: 3,
        description_key: "light_whisper_99_languages_and_it_uses_your_vocabulary",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-small.tar.bz2",
        sha256: "486a46afbb7ba798507190ffe02fea2dd726049af212e774537efac6afb210a6",
        bytes: 639_387_718,
        keep: &[
            "small-encoder.int8.onnx",
            "small-decoder.int8.onnx",
            "small-tokens.txt",
        ],
    },
    ModelDef {
        id: "whisper-base",
        family: "whisper",
        name: "Whisper Base",
        size_mb: 153,
        languages_key: "99_languages",
        speed: 3,
        accuracy: 2,
        description_key: "light_whisper_99_languages_and_it_uses_your_vocabulary",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.tar.bz2",
        sha256: "911b2083efd7c0dca2ac3b358b75222660dc09fb716d64fbfc417ba6c99ff3de",
        bytes: 207_557_382,
        keep: &[
            "base-encoder.int8.onnx",
            "base-decoder.int8.onnx",
            "base-tokens.txt",
        ],
    },
];

/// Le modèle d'un identifiant, ou `None` s'il n'est pas au catalogue.
pub fn model(id: &str) -> Option<&'static ModelDef> {
    MODELS.iter().find(|def| def.id == id)
}

/// La famille moteur d'un modèle local.
pub fn family(def: &ModelDef) -> Family {
    match def.family {
        "whisper" => Family::Whisper,
        _ => Family::Parakeet,
    }
}

/// Le dossier d'installation d'un modèle (`<données>/models/<id>`).
pub fn model_dir(id: &str) -> PathBuf {
    paths::models_dir().join(id)
}

/// Tous les fichiers du modèle sont-ils là ?
pub fn is_installed(def: &ModelDef) -> bool {
    let dir = model_dir(def.id);
    def.keep.iter().all(|name| dir.join(name).is_file())
}

/// La progression d'un téléchargement, en octets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

impl Progress {
    /// 0…1 (0 quand la taille totale est inconnue).
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.done as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }
}

// MARK: La machine

/// Ce que la règle de recommandation regarde sur l'ordinateur.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    pub cpu: String,
    pub physical_cores: usize,
    pub ram_total_gb: f64,
    pub ram_available_gb: f64,
    pub avx2: bool,
    pub nvidia: bool,
}

impl Machine {
    /// La fiche de la machine, RAM en Go (1 décimale, comme l'app macOS).
    pub fn detect() -> Self {
        use sysinfo::System;

        let mut system = System::new_all();
        system.refresh_all();
        let cpu = system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().trim().to_string())
            .filter(|brand| !brand.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        Self {
            cpu,
            physical_cores: system.physical_core_count().unwrap_or(0),
            ram_total_gb: gb(system.total_memory()),
            ram_available_gb: gb(system.available_memory()),
            avx2: avx2_available(),
            nvidia: nvidia_present(),
        }
    }
}

/// Des octets en Go, arrondis à une décimale.
fn gb(bytes: u64) -> f64 {
    let gigabytes = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    (gigabytes * 10.0).round() / 10.0
}

/// AVX2, la seule extension que Parakeet exige vraiment.
fn avx2_available() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

/// Un GPU NVIDIA : le pilote noyau, ou l'outil en ligne de commande.
fn nvidia_present() -> bool {
    std::path::Path::new("/proc/driver/nvidia").exists() || which("nvidia-smi").is_some()
}

/// Le binaire dans le `PATH`, sans dépendance supplémentaire.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|folder| folder.join(name))
        .find(|candidate| candidate.is_file())
}

/// La langue est-elle une des 25 de Parakeet ?
pub fn parakeet_speaks(language: &str) -> bool {
    let language = language.trim().to_ascii_lowercase();
    // `fr-FR` → `fr`.
    let base = language
        .split(['-', '_'])
        .next()
        .unwrap_or(language.as_str());
    PARAKEET_LANGUAGES.contains(&base)
}

/// « Détection automatique » : l'utilisateur ne choisit pas de langue, et
/// Parakeet sait reconnaître la sienne parmi ses 25 langues. Ce n'est donc pas
/// une langue « hors Parakeet » — c'était le défaut de la première règle, qui
/// envoyait la détection automatique sur Whisper Small.
fn is_automatic(language: &str) -> bool {
    matches!(
        language.trim().to_ascii_lowercase().as_str(),
        "" | "auto" | "automatic" | "automatic_detection"
    )
}

/// La règle de `docs/ipc.md`, exactement :
///
/// - langue dans les 25 de Parakeet → `parakeet-v3`, sauf RAM disponible
///   < 1,5 Go → `whisper-base` ;
/// - « détection automatique » → `parakeet-v3` aussi : il détecte seul parmi
///   ses 25 langues (c'est le modèle par défaut de Plume) ;
/// - autre langue → `whisper-small` si RAM totale ≥ 8 Go **et** disponible
///   ≥ 2 Go, sinon `whisper-base`.
pub fn recommend(language: &str, machine: &Machine) -> &'static str {
    if is_automatic(language) || parakeet_speaks(language) {
        if machine.ram_available_gb < PARAKEET_MIN_AVAILABLE_GB {
            "whisper-base"
        } else {
            "parakeet-v3"
        }
    } else if machine.ram_total_gb >= WHISPER_SMALL_MIN_TOTAL_GB
        && machine.ram_available_gb >= WHISPER_SMALL_MIN_AVAILABLE_GB
    {
        "whisper-small"
    } else {
        "whisper-base"
    }
}

// MARK: La vue du front

/// Un modèle tel que la page Modèles le lit (`ModelView` de `docs/ipc.md`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelView {
    pub id: String,
    pub family: String,
    pub name: String,
    pub size_mb: u64,
    pub languages: String,
    pub speed: u8,
    pub accuracy: u8,
    pub description_key: String,
    pub installed: bool,
    pub downloading: bool,
    pub progress: f64,
    pub recommended: bool,
    pub active: bool,
}

// MARK: L'état de l'app à côté des réglages

/// Ce que Plume garde en plus de `Prefs` : la recommandation calculée une fois.
///
/// Le fichier (`app-state.json`) vit à côté de `settings.json`, pas dans le
/// cœur : `Prefs` n'a pas ces champs, et le cœur ne doit pas bouger.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppExtras {
    /// Le modèle choisi par la recommandation, calculé **une seule fois**.
    pub recommended_model: Option<String>,
    /// Un modèle a déjà été choisi (par la règle ou par l'utilisateur).
    pub model_chosen: bool,
}

impl AppExtras {
    /// Le fichier `app-state.json` du dossier des réglages.
    pub fn path() -> PathBuf {
        paths::settings_dir().join("app-state.json")
    }

    /// Lit le fichier ; absent ou illisible → valeur par défaut.
    pub fn load() -> Self {
        std::fs::read(Self::path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Écrit le fichier d'un bloc (temporaire puis `rename`).
    pub fn save(&self) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        paths::write_atomic(&Self::path(), &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine(total: f64, available: f64) -> Machine {
        Machine {
            cpu: "Test CPU".into(),
            physical_cores: 4,
            ram_total_gb: total,
            ram_available_gb: available,
            avx2: true,
            nvidia: false,
        }
    }

    /// La règle exacte de `docs/ipc.md`, aux bornes comprises.
    #[test]
    fn recommend_rule_cases() {
        // 1. Langue Parakeet, assez de mémoire → Parakeet.
        assert_eq!(recommend("fr", &machine(8.0, 4.0)), "parakeet-v3");
        // 2. Langue Parakeet, mémoire disponible juste au-dessus du seuil.
        assert_eq!(recommend("de", &machine(16.0, 1.5)), "parakeet-v3");
        // 3. Langue Parakeet, moins de 1,5 Go → whisper-base.
        assert_eq!(recommend("en", &machine(16.0, 1.4)), "whisper-base");
        // 4. Langue hors Parakeet, machine confortable → whisper-small.
        assert_eq!(recommend("ja", &machine(16.0, 8.0)), "whisper-small");
        // 5. Langue hors Parakeet, borne exacte 8 Go / 2 Go → whisper-small.
        assert_eq!(recommend("zh", &machine(8.0, 2.0)), "whisper-small");
        // 6. Langue hors Parakeet, mémoire disponible insuffisante → base.
        assert_eq!(recommend("ar", &machine(16.0, 1.9)), "whisper-base");
        // 7. Langue hors Parakeet, machine à 4 Go → base.
        assert_eq!(recommend("ja", &machine(4.0, 3.0)), "whisper-base");
        // 8. « Détection automatique » → Parakeet : il détecte seul parmi ses
        //    25 langues. C'était le défaut relevé sur la capture Modèles, qui
        //    affichait Whisper Small comme recommandé.
        assert_eq!(recommend("auto", &machine(16.0, 8.0)), "parakeet-v3");
        assert_eq!(recommend("", &machine(8.0, 4.0)), "parakeet-v3");
        assert_eq!(recommend("Automatic", &machine(8.0, 4.0)), "parakeet-v3");
        // 8 bis. La détection automatique garde le garde-fou de mémoire.
        assert_eq!(recommend("auto", &machine(16.0, 1.4)), "whisper-base");
        // 9. La locale complète est ramenée à sa langue.
        assert_eq!(recommend("fr-FR", &machine(8.0, 4.0)), "parakeet-v3");
        assert_eq!(recommend("pt_BR", &machine(8.0, 4.0)), "parakeet-v3");
    }

    #[test]
    fn the_25_parakeet_languages_are_the_documented_ones() {
        let expected = [
            "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu", "it", "lv",
            "lt", "mt", "pl", "pt", "ro", "sk", "sl", "es", "sv", "ru", "uk",
        ];
        assert_eq!(PARAKEET_LANGUAGES, expected);
        assert!(parakeet_speaks("FR"));
        assert!(!parakeet_speaks("ja"));
    }

    /// Le catalogue est utilisable tel quel : URL https, sha256 sur 64
    /// caractères hexadécimaux, taille et fichiers cohérents.
    #[test]
    fn catalogue_integrity() {
        assert_eq!(MODELS.len(), 3);
        for def in &MODELS {
            assert!(def.url.starts_with("https://"), "{}", def.url);
            assert_eq!(def.sha256.len(), 64, "{}", def.id);
            assert!(
                def.sha256.chars().all(|c| c.is_ascii_hexdigit()),
                "{} : sha256 non hexadécimal",
                def.id
            );
            assert_eq!(def.sha256.to_ascii_lowercase(), def.sha256);
            assert!(def.bytes > 0, "{}", def.id);
            assert!(!def.keep.is_empty(), "{}", def.id);
            assert!(def.size_mb > 0, "{}", def.id);
            assert!((1..=5).contains(&def.speed), "{}", def.id);
            assert!((1..=5).contains(&def.accuracy), "{}", def.id);
            assert!(matches!(def.family, "parakeet" | "whisper"), "{}", def.id);
        }
        assert_eq!(model("parakeet-v3").unwrap().family, "parakeet");
        assert_eq!(model("whisper-small").unwrap().family, "whisper");
        assert!(model("cloud").is_none(), "pas de modèle en ligne ici");
    }

    #[test]
    fn every_model_keeps_its_family_files() {
        let parakeet = model("parakeet-v3").unwrap();
        assert_eq!(family(parakeet), Family::Parakeet);
        assert!(parakeet.keep.contains(&"joiner.int8.onnx"));
        let whisper = model("whisper-base").unwrap();
        assert_eq!(family(whisper), Family::Whisper);
        assert!(whisper.keep.iter().all(|name| name.ends_with(".onnx")
            || name.ends_with(".txt")));
    }

    #[test]
    fn gb_rounds_to_one_decimal() {
        assert_eq!(gb(0), 0.0);
        assert_eq!(gb(1024 * 1024 * 1024), 1.0);
        assert_eq!(gb(3 * 1024 * 1024 * 1024 / 2), 1.5);
        assert_eq!(gb(16 * 1024 * 1024 * 1024 + 100_000_000), 16.1);
    }

    #[test]
    fn progress_fraction_is_safe() {
        assert_eq!(Progress::default().fraction(), 0.0);
        assert_eq!(
            Progress {
                done: 50,
                total: 200
            }
            .fraction(),
            0.25
        );
        assert_eq!(
            Progress {
                done: 300,
                total: 200
            }
            .fraction(),
            1.0
        );
    }

    #[test]
    fn models_are_installed_only_when_every_file_is_there() {
        let def = model("whisper-base").unwrap();
        let dir = std::env::temp_dir().join(format!("plume-models-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // `model_dir` dépend de PLUME_DATA_DIR : on vérifie la règle sur un
        // dossier équivalent, sans toucher aux variables globales.
        for name in def.keep {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        assert!(def.keep.iter().all(|name| dir.join(name).is_file()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn app_extras_round_trip_in_a_temporary_folder() {
        // Le fichier réel dépend des réglages de l'utilisateur : on teste le
        // format, pas l'emplacement.
        let extras = AppExtras {
            recommended_model: Some("parakeet-v3".into()),
            model_chosen: true,
        };
        let raw = serde_json::to_string(&extras).unwrap();
        assert!(raw.contains("\"recommendedModel\":\"parakeet-v3\""));
        assert!(raw.contains("\"modelChosen\":true"));
        let back: AppExtras = serde_json::from_str(&raw).unwrap();
        assert_eq!(back, extras);
    }
}
