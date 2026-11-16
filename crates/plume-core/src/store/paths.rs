//! Où Plume range ses fichiers (portage de `Setup.dataFolder`).
//!
//! Un seul dossier pour tout quand `PLUME_DATA_DIR` est posé (tests, mode
//! portable), exactement comme l'app macOS. Sinon, les conventions du système :
//!
//! - **Linux** : les réglages dans `$XDG_CONFIG_HOME/plume` (`~/.config/plume`),
//!   les données — historique, modèles, audios à réessayer — dans
//!   `$XDG_DATA_HOME/plume` (`~/.local/share/plume`) ;
//! - **Windows** : tout dans `%APPDATA%\Plume`.
//!
//! Les variables `XDG_*` sont lues par `dirs` 5, jamais à la main.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// `PLUME_DATA_DIR` : tout au même endroit, comme le `Setup.dataFolder` de
/// l'app macOS (tests, démonstrations, mode portable).
pub const DATA_DIR_ENV: &str = "PLUME_DATA_DIR";

/// Nom du dossier de l'app : minuscule sous Linux (convention XDG), « Plume »
/// sous Windows (le nom de l'app, comme le dossier macOS).
#[cfg(windows)]
const APP_FOLDER: &str = "Plume";
#[cfg(not(windows))]
const APP_FOLDER: &str = "plume";

/// Audio des dictées ratées, pour « Réessayer » — même nom de dossier que
/// l'app macOS, pour qu'une reprise des données reste lisible.
pub const FAILED_AUDIO_FOLDER: &str = "audio-a-reessayer";

/// Les modèles téléchargés (`modelsFolder` côté macOS, « Models »).
pub const MODELS_FOLDER: &str = "models";

/// Le dossier imposé par `PLUME_DATA_DIR`, s'il est posé et non vide.
fn override_dir() -> Option<PathBuf> {
    std::env::var_os(DATA_DIR_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Les réglages (`settings.json`).
pub fn settings_dir() -> PathBuf {
    override_dir().unwrap_or_else(|| base(dirs::config_dir()).join(APP_FOLDER))
}

/// L'historique, les modèles et les audios à réessayer.
pub fn data_dir() -> PathBuf {
    override_dir().unwrap_or_else(|| base(dirs::data_dir()).join(APP_FOLDER))
}

/// Le fichier des réglages.
pub fn settings_file() -> PathBuf {
    settings_dir().join("settings.json")
}

/// L'audio des dictées ratées.
pub fn failed_audio_dir() -> PathBuf {
    data_dir().join(FAILED_AUDIO_FOLDER)
}

/// Les modèles téléchargés.
pub fn models_dir() -> PathBuf {
    data_dir().join(MODELS_FOLDER)
}

/// Repli si le système ne dit pas où est le dossier personnel (`HOME` absent) :
/// un dossier relatif au dossier courant, jamais un chemin inventé.
fn base(dir: Option<PathBuf>) -> PathBuf {
    dir.unwrap_or_else(|| {
        log::warn!("dossier personnel introuvable : repli sur le dossier courant");
        PathBuf::from(".")
    })
}

/// Crée un dossier et ses parents (`try? FileManager.createDirectory` de Swift).
pub fn ensure_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)
}

/// Écrit un fichier d'un bloc : fichier temporaire puis `rename`, pour qu'un
/// arrêt au milieu n'abîme jamais l'ancien — c'est `Data.write(options: .atomic)`
/// côté macOS.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        ensure_dir(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes)?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            // On ne laisse pas traîner le temporaire d'un échec.
            let _ = fs::remove_file(&tmp);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testing::Env;

    /// `PLUME_DATA_DIR` déplace **tout** : réglages, historique, audios.
    #[test]
    fn store_paths_plume_data_dir_prime() {
        let mut env = Env::new();
        let dir = env.data_dir();
        assert_eq!(settings_dir(), dir.path());
        assert_eq!(data_dir(), dir.path());
        assert_eq!(settings_file(), dir.path().join("settings.json"));
        assert_eq!(failed_audio_dir(), dir.path().join(FAILED_AUDIO_FOLDER));
        assert_eq!(models_dir(), dir.path().join(MODELS_FOLDER));
    }

    /// Linux : réglages et données sont séparés, sous `plume` (XDG).
    #[cfg(not(windows))]
    #[test]
    fn store_paths_suivent_xdg() {
        let mut env = Env::new();
        env.unset(DATA_DIR_ENV);
        let root = tempfile::tempdir().expect("dossier temporaire");
        let config = root.path().join("config");
        let data = root.path().join("data");
        env.set("XDG_CONFIG_HOME", &config);
        env.set("XDG_DATA_HOME", &data);
        assert_eq!(settings_dir(), config.join("plume"));
        assert_eq!(data_dir(), data.join("plume"));
        assert_eq!(settings_file(), config.join("plume").join("settings.json"));
    }

    /// Écriture atomique : le contenu est remplacé, aucun temporaire ne reste.
    #[test]
    fn store_paths_ecriture_atomique() {
        let root = tempfile::tempdir().expect("dossier temporaire");
        let file = root.path().join("sous/dossier/settings.json");
        write_atomic(&file, b"un").expect("première écriture");
        assert_eq!(fs::read(&file).expect("relu"), b"un");
        write_atomic(&file, b"deux").expect("seconde écriture");
        assert_eq!(fs::read(&file).expect("relu"), b"deux");
        let names: Vec<_> = fs::read_dir(root.path().join("sous/dossier"))
            .expect("dossier créé")
            .map(|entry| entry.expect("entrée").file_name())
            .collect();
        assert_eq!(names.len(), 1, "aucun fichier temporaire ne reste : {names:?}");
    }
}
