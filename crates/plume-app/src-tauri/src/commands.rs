//! Les commandes IPC de la coquille : réglages, historique, totaux, infos
//! machine, fenêtres et fin de l'app. Le front les appelle avec
//! `window.__TAURI__.core.invoke(...)`.

use std::path::{Path, PathBuf};

use plume_core::store::{Dictation, Prefs};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

use crate::state::AppState;

// MARK: Réglages

/// Tous les réglages (`settings.json`), relus à chaque appel.
#[tauri::command]
pub fn get_prefs(state: State<'_, AppState>) -> Prefs {
    state.prefs().clone()
}

/// Fusionne un patch JSON dans les réglages, enregistre, et rend le résultat.
///
/// Le front n'envoie que ce qu'il change (`{"appLanguage":"fr"}`) : les autres
/// champs gardent leur valeur, y compris ceux qu'un ancien front ne connaît pas.
#[tauri::command]
pub fn set_prefs(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: Value,
) -> Result<Prefs, String> {
    let mut current = state.prefs();
    let previous = current.clone();
    let mut merged = serde_json::to_value(&*current)
        .map_err(|error| format!("réglages illisibles : {error}"))?;
    merge_value(&mut merged, patch);
    let next: Prefs = serde_json::from_value(merged)
        .map_err(|error| format!("réglages invalides : {error}"))?;
    next.save()
        .map_err(|error| format!("réglages non enregistrés : {error}"))?;
    *current = next.clone();
    drop(current);
    // Les autres fenêtres (la principale est créée cachée au démarrage, avant
    // l'assistant) se remettent à jour : langue, thème, état du modèle.
    let _ = app.emit("plume://prefs-changed", &next);
    if previous.app_language != next.app_language {
        crate::rebuild_tray(&app);
    }
    if crate::dictation::needs_reload(&previous, &next) {
        crate::dictation::reload(app, &next);
    }
    Ok(next)
}

/// Fusion récursive d'un patch dans une valeur JSON : les objets se
/// rejoignent, tout le reste (tableau, nombre, texte, `null`) remplace.
fn merge_value(base: &mut Value, patch: Value) {
    match (base, patch) {
        (Value::Object(base), Value::Object(patch)) => {
            for (key, value) in patch {
                match base.get_mut(&key) {
                    Some(slot) => merge_value(slot, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, patch) => *base = patch,
    }
}

// MARK: Historique

/// Les dictées, la plus récente en premier.
#[tauri::command]
pub fn get_history(state: State<'_, AppState>) -> Vec<Dictation> {
    state.history().items().to_vec()
}

/// Retire une dictée (son audio d'échec part avec elle).
#[tauri::command]
pub fn delete_dictation(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|error| format!("identifiant invalide : {error}"))?;
    state.history().delete(id);
    Ok(())
}

/// Vide l'historique ; le compteur cumulé, lui, ne baisse pas.
#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) {
    state.history().clear();
}

/// Les trois chiffres de l'Accueil : aujourd'hui, 7 jours, cumulé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TotalsView {
    pub today: usize,
    pub last7_days: usize,
    pub dictations: usize,
    pub words: usize,
}

#[tauri::command]
pub fn totals(state: State<'_, AppState>) -> TotalsView {
    let history = state.history();
    let totals = history.totals();
    TotalsView {
        today: history.words_today(),
        last7_days: history.words_last_7_days(),
        dictations: totals.dictations,
        words: totals.words,
    }
}

// MARK: Infos machine

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub os: String,
    pub cpu: String,
    pub logical_cores: usize,
    pub physical_cores: usize,
    pub avx2: bool,
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
    pub nvidia: bool,
    /// Le modèle Parakeet est là (`<données>/models/parakeet-v3/encoder.int8.onnx`).
    pub model_present: bool,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    use sysinfo::System;

    let mut system = System::new_all();
    system.refresh_all();
    let cpu = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .filter(|brand| !brand.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let os = format!(
        "{} {}",
        System::name().unwrap_or_else(|| "unknown".to_string()),
        System::os_version().unwrap_or_default()
    )
    .trim()
    .to_string();

    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        os,
        cpu,
        logical_cores: system.cpus().len(),
        physical_cores: system.physical_core_count().unwrap_or(0),
        avx2: avx2_available(),
        total_ram_bytes: system.total_memory(),
        available_ram_bytes: system.available_memory(),
        nvidia: nvidia_present(),
        model_present: model_present(),
    }
}

/// AVX2, la seule extension que le moteur Parakeet exige vraiment.
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
    Path::new("/proc/driver/nvidia").exists() || which("nvidia-smi").is_some()
}

/// Le binaire dans le `PATH`, sans dépendance supplémentaire.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|folder| folder.join(name))
        .find(|candidate| candidate.is_file())
}

/// Le dossier du modèle Parakeet, tel que le construit `dictation.rs`.
fn model_present() -> bool {
    plume_core::store::paths::models_dir()
        .join("parakeet-v3")
        .join("encoder.int8.onnx")
        .is_file()
}

// MARK: Fenêtres

/// Montre une fenêtre, ou crée « Nouveautés » à la demande.
#[tauri::command]
pub fn open_window(app: AppHandle, name: String) -> Result<(), String> {
    if name == "whatsnew" {
        if let Some(window) = app.get_webview_window("whatsnew") {
            let _ = window.show();
            let _ = window.set_focus();
            return Ok(());
        }
        WebviewWindowBuilder::new(&app, "whatsnew", WebviewUrl::App("whatsnew.html".into()))
            .title("Plume")
            .inner_size(560.0, 500.0)
            .resizable(false)
            .center()
            .build()
            .map_err(|error| format!("fenêtre Nouveautés : {error}"))?;
        return Ok(());
    }

    let window = app
        .get_webview_window(&name)
        .ok_or_else(|| format!("fenêtre inconnue : {name}"))?;
    window
        .show()
        .map_err(|error| format!("fenêtre {name} : {error}"))?;
    if name != "pill" {
        let _ = window.set_focus();
    }
    Ok(())
}

/// Donne à la fenêtre de la pastille la taille de ce qu'elle montre : la
/// pastille ordinaire (320 × 64) ou le pop-up « rien à coller » (430 × 96),
/// puis la recentre (`§3.1`). Le redimensionnement se fait ici, côté Rust, au
/// moment où le front change de carte — sans quoi la fenêtre reste à 320 pt et
/// le pop-up est rogné.
#[tauri::command]
pub fn pill_resize(app: AppHandle, preview: bool) {
    let style = app.state::<crate::state::AppState>().prefs().pill_style;
    crate::pill::place(&app, style, preview);
}

/// Quitte Plume pour de bon (la zone de notification n'est pas un Dock).
#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

/// Ce que le front a besoin de savoir de l'environnement de test.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiEnv {
    /// `PLUME_THEME=dark` (ou `light`) force `data-theme` ; sinon `null`.
    pub theme: Option<String>,
    /// `PLUME_DEMO=<état>` : la page peut jouer un état de démonstration
    /// (captures d'écran) ; sinon `null`.
    pub demo: Option<String>,
}

#[tauri::command]
pub fn ui_env() -> UiEnv {
    UiEnv {
        theme: std::env::var("PLUME_THEME")
            .ok()
            .map(|theme| theme.trim().to_ascii_lowercase())
            .filter(|theme| theme == "dark" || theme == "light"),
        demo: std::env::var("PLUME_DEMO")
            .ok()
            .map(|demo| demo.trim().to_string())
            .filter(|demo| !demo.is_empty()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Un patch ne touche que ses clés, et les objets se rejoignent.
    #[test]
    fn commands_merge_patch_fusionne_les_objets() {
        let mut base = json!({
            "appLanguage": "en",
            "pillStyle": "top",
            "nested": { "a": 1, "b": 2 }
        });
        merge_value(
            &mut base,
            json!({ "appLanguage": "fr", "nested": { "b": 3, "c": 4 } }),
        );
        assert_eq!(base["appLanguage"], "fr");
        assert_eq!(base["pillStyle"], "top", "les autres clés sont gardées");
        assert_eq!(base["nested"]["a"], 1);
        assert_eq!(base["nested"]["b"], 3);
        assert_eq!(base["nested"]["c"], 4);
    }

    /// Un tableau ou un `null` remplace au lieu de fusionner.
    #[test]
    fn commands_merge_patch_remplace_le_reste() {
        let mut base = json!({ "list": [1, 2], "value": { "a": 1 } });
        merge_value(&mut base, json!({ "list": [3], "value": null }));
        assert_eq!(base["list"], json!([3]));
        assert!(base["value"].is_null());
    }

    #[test]
    fn commands_which_trouve_un_binaire_du_path() {
        let found = which("sh");
        assert!(found.is_some(), "sh est dans le PATH");
        assert!(found.expect("sh").is_file());
        assert!(which("plume-binaire-qui-n-existe-pas").is_none());
    }
}
