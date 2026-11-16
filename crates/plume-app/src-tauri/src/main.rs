//! Plume — coquille Tauri 2 : fenêtres, zone de notification, IPC, et la
//! dictée branchée sur `plume-core`.

mod commands;
mod dictation;
mod extra;
mod models;
mod pill;
mod state;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

use state::AppState;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut builder = tauri::Builder::default();

    // L'instance unique a besoin d'un bus de session sous Linux ; sans lui
    // (Xvfb nu), on ne l'installe pas et l'app démarre quand même. Ailleurs
    // (Windows), elle est toujours là.
    let has_session_bus = !cfg!(target_os = "linux")
        || std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some();
    if has_session_bus {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_window(app, "main");
        }));
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .setup(|app| {
            let state = AppState::load();
            let prefs = state.prefs().clone();
            let mut mutable_prefs = state.prefs();
            let onboarding = mutable_prefs.onboarding_needed();
            drop(mutable_prefs);
            app.manage(state);

            // La zone de notification est toujours là sous Linux (pas de Dock).
            if let Err(error) = build_tray(app.handle()) {
                log::warn!("zone de notification indisponible : {error}");
            }

            // Démarrage automatique : l'état vient des réglages.
            {
                use tauri_plugin_autostart::ManagerExt;
                let autolaunch = app.autolaunch();
                let result = if prefs.launch_at_login {
                    autolaunch.enable()
                } else {
                    autolaunch.disable()
                };
                if let Err(error) = result {
                    log::warn!("démarrage automatique : {error}");
                }
            }

            // La dictée : le chef d'orchestre du cœur, ses événements, l'historique.
            dictation::spawn(app.handle().clone(), &prefs);

            // La pastille : taille et position de repos (style des réglages).
            let style = app.state::<AppState>().prefs().pill_style;
            pill::place(app.handle(), style, false);

            // Quelle fenêtre montrer au démarrage : `PLUME_SHOW` (tests) sinon
            // l'assistant au premier lancement, sinon rien (zone de notification).
            match show_target(std::env::var("PLUME_SHOW").ok().as_deref()) {
                Some("whatsnew") => {
                    if let Err(error) = commands::open_window(app.handle().clone(), "whatsnew".into())
                    {
                        log::warn!("fenêtre Nouveautés : {error}");
                    }
                }
                Some(label) => show_window(app.handle(), label),
                None if onboarding => show_window(app.handle(), "onboarding"),
                None => {}
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_prefs,
            commands::set_prefs,
            commands::get_history,
            commands::delete_dictation,
            commands::clear_history,
            commands::totals,
            commands::app_info,
            commands::open_window,
            commands::quit,
            commands::ui_env,
            commands::pill_resize,
            extra::system_check,
            extra::list_mics,
            extra::mic_test_start,
            extra::mic_test_stop,
            extra::list_models,
            extra::recommend_model,
            extra::download_model,
            extra::cancel_download,
            extra::delete_model,
            extra::use_model,
            extra::engine_status,
            extra::get_vocabulary,
            extra::set_vocabulary,
            extra::leet_convert,
            extra::rewrite_dictation,
            extra::retry_dictation,
            extra::copy_text,
            extra::polish_test,
            extra::set_dictation_key_capture,
            extra::get_whats_new,
            extra::mark_whats_new_seen,
        ])
        .run(tauri::generate_context!())
        .expect("Plume n'a pas pu démarrer");
}

/// `PLUME_SHOW=main` / `onboarding` / `pill` : la fenêtre à montrer au
/// démarrage (tests de capture). Toute autre valeur est ignorée.
fn show_target(value: Option<&str>) -> Option<&'static str> {
    match value.map(str::trim) {
        Some("main") => Some("main"),
        Some("onboarding") => Some("onboarding"),
        Some("whatsnew") => Some("whatsnew"),
        Some("pill") => Some("pill"),
        _ => None,
    }
}

/// Montre et met au premier plan (sans voler le focus pour la pastille).
fn show_window(app: &AppHandle, label: &str) {
    let Some(window) = app.get_webview_window(label) else {
        log::warn!("fenêtre inconnue : {label}");
        return;
    };
    if let Err(error) = window.show() {
        log::warn!("fenêtre {label} : {error}");
    }
    if label != "pill" {
        let _ = window.set_focus();
    }
}

/// La zone de notification : les entrées de `§4`, dans cet ordre.
fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let start = MenuItem::with_id(app, "dictation", "Start Dictation", true, None::<&str>)?;
    let copy = MenuItem::with_id(app, "copy", "Copy Last Dictation", true, None::<&str>)?;
    let history = MenuItem::with_id(app, "history", "History…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let support = MenuItem::with_id(app, "support", "Support Plume…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Plume", true, None::<&str>)?;
    let separators = [
        PredefinedMenuItem::separator(app)?,
        PredefinedMenuItem::separator(app)?,
    ];

    let menu = Menu::new(app)?;
    menu.append(&start)?;
    menu.append(&copy)?;
    menu.append(&separators[0])?;
    menu.append(&history)?;
    menu.append(&settings)?;
    menu.append(&support)?;
    menu.append(&separators[1])?;
    menu.append(&quit)?;

    TrayIconBuilder::with_id("plume-tray")
        .icon(tauri::include_image!("../icons/tray.png"))
        .tooltip("Plume")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "dictation" => {
                // La touche globale déclenche la dictée ; cette entrée prévient
                // le front (la pastille de cette tâche est un placeholder).
                let _ = app.emit("plume://tray", "dictation");
            }
            "copy" => copy_last_dictation(app),
            "history" => open_page(app, "history"),
            "settings" => open_page(app, "settings"),
            "support" => {
                use tauri_plugin_opener::OpenerExt;
                let opener = app.opener();
                if let Err(error) = opener.open_url("https://buymeacoffee.com/ppocass", None::<&str>)
                {
                    log::warn!("page de soutien : {error}");
                }
            }
            "quit" => app.exit(0),
            other => log::debug!("entrée de menu inconnue : {other}"),
        })
        .build(app)?;

    Ok(())
}

/// « Copier la dernière dictée » : le texte part dans le presse-papiers.
fn copy_last_dictation(app: &AppHandle) {
    let text = app
        .state::<AppState>()
        .history()
        .latest()
        .map(|dictation| dictation.text.clone());
    let Some(text) = text else {
        return;
    };
    match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text)) {
        Ok(()) => log::info!("dernière dictée copiée"),
        Err(error) => log::warn!("presse-papiers : {error}"),
    }
}

/// Ouvre la fenêtre principale sur une page (`history`, `settings`).
fn open_page(app: &AppHandle, page: &str) {
    show_window(app, "main");
    let _ = app.emit("plume://navigate", page);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_plume_show_ne_garde_que_les_fenetres_connues() {
        assert_eq!(show_target(Some("main")), Some("main"));
        assert_eq!(show_target(Some(" onboarding ")), Some("onboarding"));
        assert_eq!(show_target(Some("whatsnew")), Some("whatsnew"));
        assert_eq!(show_target(Some("pill")), Some("pill"));
        assert_eq!(show_target(Some("nawak")), None);
        assert_eq!(show_target(None), None);
    }
}
