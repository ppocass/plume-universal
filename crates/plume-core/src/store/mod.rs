//! Module store : ce que Plume garde d'une session à l'autre — les réglages
//! (`settings.json`) et l'historique des dictées (`historique.json`), avec les
//! audios des dictées ratées mis de côté pour « Réessayer ».
//!
//! Portage fidèle de `Prefs.swift` et `History.swift` de l'app macOS : mêmes
//! noms de champs JSON, mêmes limites, mêmes réparations silencieuses. Seul le
//! support change (des fichiers JSON au lieu des `UserDefaults` et du dossier
//! Application Support).
//!
//! Tous les tests passent par `PLUME_DATA_DIR` (voir `testing`) : ils écrivent
//! dans un dossier temporaire, jamais dans le vrai dossier de l'utilisateur.

pub mod history;
pub mod paths;
pub mod prefs;

pub use history::{Dictation, History, Totals};
pub use prefs::Prefs;

use serde::Serialize;
use std::io;

/// Les dates écrites par Plume sont en ISO 8601 UTC, sans fraction de seconde :
/// exactement ce que produit `JSONEncoder.dateEncodingStrategy = .iso8601` côté
/// macOS, si bien que les deux apps relisent les mêmes fichiers.
pub(crate) mod iso {
    use chrono::{DateTime, SecondsFormat, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub(crate) fn format(date: DateTime<Utc>) -> String {
        date.to_rfc3339_opts(SecondsFormat::Secs, true)
    }

    /// Lit une date ISO 8601 (fuseau et fraction de seconde acceptés).
    pub(crate) fn parse(raw: &str) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(raw)
            .ok()
            .map(|date| date.with_timezone(&Utc))
    }

    pub(crate) fn serialize<S: Serializer>(
        date: &DateTime<Utc>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format(*date))
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<DateTime<Utc>, D::Error> {
        let raw = String::deserialize(deserializer)?;
        parse(&raw)
            .ok_or_else(|| serde::de::Error::custom(format!("date ISO 8601 attendue, reçu {raw:?}")))
    }

    /// Le même couple, pour un champ facultatif (`null` → `None`).
    pub(crate) mod optional {
        use chrono::{DateTime, Utc};
        use serde::{Deserialize, Deserializer, Serializer};

        pub(crate) fn serialize<S: Serializer>(
            date: &Option<DateTime<Utc>>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            match date {
                Some(date) => serializer.serialize_str(&super::format(*date)),
                None => serializer.serialize_none(),
            }
        }

        pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<DateTime<Utc>>, D::Error> {
            match Option::<String>::deserialize(deserializer)? {
                None => Ok(None),
                Some(raw) => super::parse(&raw).map(Some).ok_or_else(|| {
                    serde::de::Error::custom(format!("date ISO 8601 attendue, reçu {raw:?}"))
                }),
            }
        }
    }
}

/// Du JSON lisible (`outputFormatting = [.prettyPrinted]` côté macOS).
pub(crate) fn to_json_pretty<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    serde_json::to_vec_pretty(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Ce qu'il faut aux tests pour ne jamais toucher au vrai dossier : un
/// `PLUME_DATA_DIR` temporaire, posé sous verrou.
///
/// ⚠️ Les variables d'environnement sont globales au processus et les tests
/// tournent en parallèle : un seul test à la fois peut les changer, d'où le
/// verrou pris par `Env`.
#[cfg(test)]
pub(crate) mod testing {
    use std::ffi::{OsStr, OsString};
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use tempfile::TempDir;

    fn lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        // Un test qui panique empoisonne le verrou : on reprend la main, les
        // tests suivants ne dépendent pas de son état.
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Un test qui touche aux variables d'environnement ; restaure tout en
    /// sortant, dans l'ordre inverse.
    pub(crate) struct Env {
        _lock: MutexGuard<'static, ()>,
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    impl Env {
        pub(crate) fn new() -> Self {
            Self {
                _lock: lock(),
                saved: Vec::new(),
            }
        }

        fn remember(&mut self, key: &'static str) {
            if !self.saved.iter().any(|(known, _)| *known == key) {
                self.saved.push((key, std::env::var_os(key)));
            }
        }

        pub(crate) fn set(&mut self, key: &'static str, value: impl AsRef<OsStr>) {
            self.remember(key);
            std::env::set_var(key, value);
        }

        pub(crate) fn unset(&mut self, key: &'static str) {
            self.remember(key);
            std::env::remove_var(key);
        }

        /// Un dossier de données temporaire, posé dans `PLUME_DATA_DIR` : le
        /// seul endroit où les tests lisent et écrivent.
        pub(crate) fn data_dir(&mut self) -> TempDir {
            let dir = tempfile::tempdir().expect("dossier temporaire");
            self.set(super::paths::DATA_DIR_ENV, dir.path());
            dir
        }
    }

    impl Drop for Env {
        fn drop(&mut self) {
            for (key, value) in self.saved.drain(..).rev() {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}
