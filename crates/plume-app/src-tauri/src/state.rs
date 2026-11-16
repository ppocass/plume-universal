//! L'état partagé de l'app : les réglages et l'historique, chacun derrière un
//! verrou — les mêmes fichiers JSON que l'app macOS, via `plume-core::store`.

use std::sync::{Mutex, MutexGuard};

use plume_core::store::{History, Prefs};

pub struct AppState {
    pub prefs: Mutex<Prefs>,
    pub history: Mutex<History>,
}

impl AppState {
    /// Ouvre `settings.json`, `historique.json` et `totals.json` du dossier de
    /// données (`PLUME_DATA_DIR` s'il est posé).
    pub fn load() -> Self {
        Self {
            prefs: Mutex::new(Prefs::load()),
            history: Mutex::new(History::open()),
        }
    }

    /// Le verrou des réglages, même si un fil a paniqué en le tenant.
    pub fn prefs(&self) -> MutexGuard<'_, Prefs> {
        self.prefs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Le verrou de l'historique, même si un fil a paniqué en le tenant.
    pub fn history(&self) -> MutexGuard<'_, History> {
        self.history
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
