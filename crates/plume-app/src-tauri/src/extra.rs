//! Les commandes IPC de `docs/ipc.md` qui ne sont pas déjà dans `commands.rs` :
//! diagnostic système, micros, catalogue et téléchargement des modèles,
//! vocabulaire, 1337, réécriture, réessai, presse-papiers, lissage, capture de
//! touche et « Nouveautés ».
//!
//! Tout ce qui est long (téléchargement, chargement du moteur, test du micro)
//! tourne dans un fil et parle au front par événements (`plume://…`) : une
//! commande Tauri rend la main tout de suite.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use plume_core::controller::ParakeetTranscriber;
use plume_core::engine::{transcribe_dictation, Engine};
use plume_core::input::gesture::Key;
use plume_core::input::mic::{self, Mic};
use plume_core::store::prefs::{DictationLanguage, LeetLevel};
use plume_core::store::{paths, Dictation, Prefs};
use plume_core::text::{Level, Leet, VocabEntry, Vocabulary};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::models::{self, ModelDef, ModelView, Progress};
use crate::state::AppState;

// MARK: Événements

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelProgressEvent {
    id: String,
    done_bytes: u64,
    total_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelDoneEvent {
    id: String,
    ok: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MicLevelEvent {
    level: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KeyCapturedEvent {
    name: String,
}

// MARK: État partagé des commandes

fn downloads() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    static DOWNLOADS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    DOWNLOADS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn progress() -> &'static Mutex<HashMap<String, Progress>> {
    static PROGRESS: OnceLock<Mutex<HashMap<String, Progress>>> = OnceLock::new();
    PROGRESS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Le verrou, même après la panique d'un fil.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct RunningMic {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

fn mic_test() -> &'static Mutex<Option<RunningMic>> {
    static MIC_TEST: OnceLock<Mutex<Option<RunningMic>>> = OnceLock::new();
    MIC_TEST.get_or_init(|| Mutex::new(None))
}

/// Une capture de touche est armée (les événements écoutent).
static CAPTURE_ARMED: AtomicBool = AtomicBool::new(false);
/// L'écouteur de capture n'est lancé qu'une fois pour la vie de l'app.
static CAPTURE_LISTENER: OnceLock<()> = OnceLock::new();

// MARK: Système

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemCheck {
    pub session_type: &'static str,
    pub paste_tool: bool,
    pub paste_tool_name: &'static str,
    pub mic_count: usize,
}

/// Compte les micros d'entrée (jamais de nom vide).
pub fn mic_names() -> Vec<String> {
    let mut names = Vec::new();
    for name in mic::list_devices() {
        let name = name.trim().to_string();
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

#[tauri::command]
pub fn system_check() -> SystemCheck {
    let (session_type, tool) = session_and_paste_tool();
    SystemCheck {
        session_type,
        paste_tool: tool.is_some(),
        paste_tool_name: tool.unwrap_or(""),
        mic_count: mic_names().len(),
    }
}

/// Le type de session et l'outil de collage attendu sur cette plateforme.
#[cfg(windows)]
fn session_and_paste_tool() -> (&'static str, Option<&'static str>) {
    ("windows", Some("SendInput"))
}

#[cfg(not(windows))]
fn session_and_paste_tool() -> (&'static str, Option<&'static str>) {
    let xdg = std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let session_type = if xdg.contains("wayland")
        || (xdg.is_empty() && std::env::var_os("WAYLAND_DISPLAY").is_some())
    {
        "wayland"
    } else if xdg.contains("x11") || std::env::var_os("DISPLAY").is_some() {
        "x11"
    } else {
        "other"
    };
    let (tool, name) = if session_type == "wayland" {
        (which("ydotool"), "ydotool")
    } else if session_type == "x11" {
        (which("xdotool"), "xdotool")
    } else {
        (None, "")
    };
    (session_type, tool.map(|_| name))
}

/// Le binaire dans le `PATH`.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|folder| folder.join(name))
        .find(|candidate| candidate.is_file())
}

#[tauri::command]
pub fn list_mics() -> Vec<String> {
    mic_names()
}

#[tauri::command]
pub fn mic_test_start(app: AppHandle, device: Option<String>) -> Result<(), String> {
    stop_mic_test();

    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);
    let app_thread = app.clone();
    let handle = std::thread::Builder::new()
        .name("plume-test-micro".into())
        .spawn(move || {
            let mut mic = match device {
                Some(name) => Mic::with_device(name),
                None => Mic::new(),
            };
            match mic.start() {
                Ok(()) => {
                    let _ = ready_tx.send(Ok(()));
                    while !stop_thread.load(Ordering::SeqCst) {
                        let _ = app_thread.emit(
                            "plume://mic-level",
                            MicLevelEvent { level: mic.level() },
                        );
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    let _ = mic.stop();
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(format!("micro : {error:#}")));
                }
            }
        })
        .map_err(|error| format!("fil du micro : {error}"))?;

    match ready_rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(())) => {
            *lock(mic_test()) = Some(RunningMic {
                stop,
                handle: Some(handle),
            });
            Ok(())
        }
        Ok(Err(error)) => {
            let _ = handle.join();
            Err(error)
        }
        Err(_) => {
            stop.store(true, Ordering::SeqCst);
            let _ = handle.join();
            Err("micro : pas de réponse".into())
        }
    }
}

#[tauri::command]
pub fn mic_test_stop() {
    stop_mic_test();
}

fn stop_mic_test() {
    let running = lock(mic_test()).take();
    if let Some(mut running) = running {
        running.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = running.handle.take() {
            let _ = handle.join();
        }
    }
}

// MARK: Catalogue

#[tauri::command]
pub fn list_models(state: State<'_, AppState>) -> Vec<ModelView> {
    let prefs = state.prefs().clone();
    let extras = models::AppExtras::load();
    let active_downloads: Vec<String> = lock(downloads()).keys().cloned().collect();
    let progress = lock(progress()).clone();

    models::MODELS
        .iter()
        .map(|def| {
            let downloading = active_downloads.iter().any(|id| id == def.id);
            let progress = progress.get(def.id).copied().unwrap_or_default();
            ModelView {
                id: def.id.to_string(),
                family: def.family.to_string(),
                name: def.name.to_string(),
                size_mb: def.size_mb,
                languages: def.languages_key.to_string(),
                speed: def.speed,
                accuracy: def.accuracy,
                description_key: def.description_key.to_string(),
                installed: models::is_installed(def),
                downloading,
                progress: progress.fraction(),
                recommended: extras.recommended_model.as_deref() == Some(def.id),
                active: prefs.model == def.id,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub id: String,
    pub reason_key: String,
    pub machine: models::Machine,
}

/// La règle de `docs/ipc.md`, calculée **une seule fois** : le résultat est
/// gardé dans `app-state.json`, et le modèle est choisi dans `settings.json`
/// si l'utilisateur n'en a pas encore choisi un.
#[tauri::command]
pub fn recommend_model(
    app: AppHandle,
    state: State<'_, AppState>,
    spoken_language: String,
) -> Recommendation {
    let machine = models::Machine::detect();
    let mut extras = models::AppExtras::load();
    let id = match extras.recommended_model.clone() {
        Some(id) if models::model(&id).is_some() => id,
        _ => {
            let id = models::recommend(&spoken_language, &machine).to_string();
            extras.recommended_model = Some(id.clone());
            if let Err(error) = extras.save() {
                log::warn!("recommandation non enregistrée : {error}");
            }
            id
        }
    };

    let mut reload_with: Option<Prefs> = None;
    if !extras.model_chosen {
        if let Some(def) = models::model(&id) {
            let mut prefs = state.prefs();
            let changed = prefs.model != def.id;
            prefs.model = def.id.to_string();
            if let Err(error) = prefs.save() {
                log::warn!("modèle recommandé non enregistré : {error}");
            }
            if changed {
                reload_with = Some(prefs.clone());
            }
            drop(prefs);
            extras.model_chosen = true;
            let _ = extras.save();
        }
    }

    // Le modèle choisi change : le chef d'orchestre repart avec, sans attendre
    // un redémarrage de l'app.
    if let Some(prefs) = reload_with {
        crate::dictation::reload(app, &prefs);
    }

    let reason_key = models::model(&id)
        .map(|def| def.description_key.to_string())
        .unwrap_or_else(|| "recommended_for_this_computer".to_string());
    Recommendation {
        id,
        reason_key,
        machine,
    }
}

// MARK: Téléchargement

/// Télécharge et installe un modèle, dans un fil ; la progression et la fin
/// passent par `plume://model-progress` / `plume://model-done`.
#[tauri::command]
pub fn download_model(app: AppHandle, id: String) -> Result<(), String> {
    let def = local_model(&id)?;
    if models::is_installed(def) {
        return Err("modèle déjà téléchargé".into());
    }

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut active = lock(downloads());
        if active.contains_key(&id) {
            return Err("téléchargement déjà en cours".into());
        }
        active.insert(id.clone(), Arc::clone(&cancel));
    }
    lock(progress()).insert(id.clone(), Progress::default());

    let thread_id = id.clone();
    let app_thread = app.clone();
    let spawned = std::thread::Builder::new()
        .name("plume-modele".into())
        .spawn(move || {
            let result = download_and_install(&app_thread, def, &cancel);
            lock(downloads()).remove(&thread_id);
            lock(progress()).remove(&thread_id);

            let (ok, error) = match result {
                Ok(()) => (true, None),
                Err(error) => (false, Some(error)),
            };
            let _ = app_thread.emit(
                "plume://model-done",
                ModelDoneEvent {
                    id: thread_id.clone(),
                    ok,
                    error,
                },
            );
            emit_engine_status(&app_thread);
        });
    if let Err(error) = spawned {
        lock(downloads()).remove(&id);
        lock(progress()).remove(&id);
        return Err(format!("fil de téléchargement : {error}"));
    }

    emit_engine_status(&app);
    Ok(())
}

/// Le modèle local d'un identifiant (une erreur lisible sinon).
fn local_model(id: &str) -> Result<&'static ModelDef, String> {
    models::model(id).ok_or_else(|| format!("modèle inconnu : {id}"))
}

/// Le téléchargement complet d'un modèle, avec vérification et installation.
fn download_and_install(
    app: &AppHandle,
    def: &'static ModelDef,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let part = paths::models_dir().join(format!(".{}.part", def.id));
    if let Some(parent) = part.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("dossier des modèles : {error}"))?;
    }

    let mut last_emit = Instant::now()
        .checked_sub(Duration::from_secs(1))
        .unwrap_or_else(Instant::now);
    let id = def.id.to_string();
    let mut on_progress = |done: u64, total: u64| {
        let total = if total == 0 { def.bytes } else { total };
        lock(progress()).insert(id.clone(), Progress { done, total });
        // Au plus 10 événements par seconde.
        if last_emit.elapsed() >= Duration::from_millis(120) || done >= total {
            last_emit = Instant::now();
            let _ = app.emit(
                "plume://model-progress",
                ModelProgressEvent {
                    id: id.clone(),
                    done_bytes: done,
                    total_bytes: total,
                },
            );
        }
    };

    let result = (|| -> Result<(), String> {
        fetch_archive(def, &part, cancel, &mut on_progress)?;
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled".into());
        }
        verify_sha256(&part, def.sha256)?;
        install_archive(def, &part, cancel)?;
        Ok(())
    })();

    let _ = std::fs::remove_file(&part);
    if result.is_err() {
        // Un temporaire d'extraction ne doit pas survivre à un échec.
        let _ = std::fs::remove_dir_all(paths::models_dir().join(format!(".{}.tmp", def.id)));
    }
    result
}

/// Télécharge l'archive en flux dans `part`, en rendant la main au fil souvent
/// pour écouter l'annulation.
fn fetch_archive(
    def: &ModelDef,
    part: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<(), String> {
    let response = ureq::get(def.url)
        .call()
        .map_err(|error| format!("téléchargement impossible : {error}"))?;
    let total = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(def.bytes);
    let mut reader = response.into_reader();
    let mut file = File::create(part).map_err(|error| format!("fichier .part : {error}"))?;
    let mut buffer = vec![0u8; 64 * 1024];
    let mut done = 0u64;

    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled".into());
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("téléchargement interrompu : {error}"))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|error| format!("écriture du modèle : {error}"))?;
        done += read as u64;
        on_progress(done, total);
    }
    file.flush().map_err(|error| format!("écriture du modèle : {error}"))?;
    on_progress(done, total);
    if done != total {
        return Err(format!(
            "téléchargement incomplet ({done} octets sur {total})"
        ));
    }
    Ok(())
}

/// Le sha256 du fichier, en minuscules.
fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| format!("modèle illisible : {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("modèle illisible : {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)
}

fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    let actual = sha256_file(path)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!(
            "fichier corrompu : sha256 {actual} au lieu de {expected}"
        ))
    }
}

/// Extrait l'archive dans un dossier temporaire, ne garde que les fichiers du
/// modèle, puis renomme le dossier en une seule opération.
fn install_archive(
    def: &ModelDef,
    archive: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let root_dir = paths::models_dir();
    let temp = root_dir.join(format!(".{}.tmp", def.id));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).map_err(|error| format!("dossier temporaire : {error}"))?;

    let file = File::open(archive).map_err(|error| format!("archive illisible : {error}"))?;
    let decoder = bzip2::read::BzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);
    tar.unpack(&temp)
        .map_err(|error| format!("extraction de l'archive : {error}"))?;
    if cancel.load(Ordering::SeqCst) {
        let _ = std::fs::remove_dir_all(&temp);
        return Err("cancelled".into());
    }

    let root = find_archive_root(&temp, def.keep).ok_or_else(|| {
        format!(
            "archive inattendue : dossier du modèle introuvable dans {}",
            def.id
        )
    })?;

    // On ne garde que les fichiers utiles : le README, les WAV de test et les
    // variantes non int8 n'ont rien à faire dans le dossier du modèle.
    let entries = std::fs::read_dir(&root)
        .map_err(|error| format!("contenu de l'archive : {error}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    for path in entries {
        let keep = path
            .file_name()
            .map(|name| {
                let name = name.to_string_lossy();
                def.keep.iter().any(|wanted| *wanted == name)
            })
            .unwrap_or(false);
        if keep {
            continue;
        }
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }

    let final_dir = models::model_dir(def.id);
    if let Some(parent) = final_dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("dossier des modèles : {error}"))?;
    }
    if final_dir.exists() {
        std::fs::remove_dir_all(&final_dir)
            .map_err(|error| format!("ancien modèle non remplacé : {error}"))?;
    }
    std::fs::rename(&root, &final_dir)
        .map_err(|error| format!("installation du modèle : {error}"))?;
    let _ = std::fs::remove_dir_all(&temp);
    Ok(())
}

/// Le dossier du modèle dans ce qu'a produit l'extraction (`sherpa-onnx-…/`).
fn find_archive_root(temp: &Path, keep: &[&str]) -> Option<PathBuf> {
    if keep.iter().all(|name| temp.join(name).is_file()) {
        return Some(temp.to_path_buf());
    }
    let entries = std::fs::read_dir(temp)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    if entries.len() == 1 && entries[0].is_dir() {
        return Some(entries[0].clone());
    }
    entries
        .into_iter()
        .find(|path| path.is_dir() && keep.iter().all(|name| path.join(name).is_file()))
}

#[tauri::command]
pub fn cancel_download(id: String) {
    if let Some(flag) = lock(downloads()).get(&id) {
        flag.store(true, Ordering::SeqCst);
    }
}

#[tauri::command]
pub fn delete_model(app: AppHandle, id: String) -> Result<(), String> {
    let def = local_model(&id)?;
    if lock(downloads()).contains_key(&id) {
        return Err("téléchargement en cours : annulez-le d'abord".into());
    }
    let dir = models::model_dir(def.id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|error| format!("modèle non supprimé : {error}"))?;
    }
    emit_engine_status(&app);
    Ok(())
}

/// Enregistre le modèle choisi et relance le chef d'orchestre.
#[tauri::command]
pub fn use_model(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let def = local_model(&id)?;
    if !models::is_installed(def) {
        return Err(format!("modèle non téléchargé : {id}"));
    }
    let prefs = {
        let mut prefs = state.prefs();
        prefs.model = def.id.to_string();
        prefs
            .save()
            .map_err(|error| format!("réglages non enregistrés : {error}"))?;
        prefs.clone()
    };
    let mut extras = models::AppExtras::load();
    extras.model_chosen = true;
    let _ = extras.save();

    crate::dictation::reload(app.clone(), &prefs);
    emit_engine_status(&app);
    Ok(())
}

// MARK: État du moteur

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub state: String,
    pub model_id: String,
}

/// L'état du moteur pour le modèle choisi.
pub fn current_engine_status(prefs: &Prefs) -> EngineStatus {
    let id = prefs.model.clone();
    let downloading = lock(downloads()).contains_key(&id);
    let installed = models::model(&id)
        .map(models::is_installed)
        .unwrap_or(false);
    let state = if downloading {
        "downloading"
    } else if !installed {
        "missing"
    } else if ParakeetTranscriber::engine_loaded() {
        "loaded"
    } else {
        "ready"
    };
    EngineStatus {
        state: state.to_string(),
        model_id: id,
    }
}

#[tauri::command]
pub fn engine_status(app: AppHandle, state: State<'_, AppState>) -> EngineStatus {
    start_status_watcher(&app);
    let prefs = state.prefs().clone();
    current_engine_status(&prefs)
}

/// Prévenir le front d'un changement d'état (téléchargement, modèle choisi).
pub fn emit_engine_status(app: &AppHandle) {
    let prefs = {
        let state = app.state::<AppState>();
        let prefs = state.prefs().clone();
        prefs
    };
    let status = current_engine_status(&prefs);
    let _ = app.emit("plume://engine-status", status);
}

/// Un fil qui surveille l'état (chargement en tâche de fond, veille du moteur)
/// et émet `plume://engine-status` à chaque changement.
fn start_status_watcher(app: &AppHandle) {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("plume-etat-moteur".into())
        .spawn(move || {
            let mut previous: Option<EngineStatus> = None;
            loop {
                std::thread::sleep(Duration::from_millis(750));
                let status = {
                    let state = app.state::<AppState>();
                    let prefs = state.prefs().clone();
                    current_engine_status(&prefs)
                };
                if previous.as_ref() != Some(&status) {
                    if previous.is_some() {
                        let _ = app.emit("plume://engine-status", &status);
                    }
                    previous = Some(status);
                }
            }
        });
}

// MARK: Vocabulaire

/// Le fichier `vocabulary.json` du dossier des réglages (format Swift : un
/// tableau de `VocabEntry`).
pub fn vocabulary_path() -> PathBuf {
    paths::settings_dir().join("vocabulary.json")
}

fn read_vocabulary() -> Vec<VocabEntry> {
    let Ok(raw) = std::fs::read_to_string(vocabulary_path()) else {
        return Vec::new();
    };
    if let Ok(entries) = serde_json::from_str::<Vec<VocabEntry>>(&raw) {
        return entries;
    }
    serde_json::from_str::<Vocabulary>(&raw)
        .map(|vocabulary| vocabulary.entries)
        .unwrap_or_default()
}

#[tauri::command]
pub fn get_vocabulary() -> Vec<VocabEntry> {
    read_vocabulary()
}

#[tauri::command]
pub fn set_vocabulary(vocabulary: Vec<VocabEntry>) -> Result<(), String> {
    let path = vocabulary_path();
    let bytes = serde_json::to_vec_pretty(&vocabulary)
        .map_err(|error| format!("vocabulaire invalide : {error}"))?;
    paths::write_atomic(&path, &bytes)
        .map_err(|error| format!("vocabulaire non enregistré : {error}"))
}

// MARK: 1337 et réécriture

/// Le niveau 1337 d'un nombre (1 = léger, 2 = moyen, 3 = complet).
fn level_from_number(level: u32) -> Level {
    match level {
        0 | 1 => Level::Light,
        2 => Level::Medium,
        _ => Level::Full,
    }
}

/// Le niveau 1337 des réglages.
fn level_from_prefs(prefs: &Prefs) -> Level {
    match prefs.leet_level {
        LeetLevel::Light => Level::Light,
        LeetLevel::Medium => Level::Medium,
        LeetLevel::Full => Level::Full,
    }
}

/// Dé-maquillage simple : les séquences longues d'abord, puis les chiffres.
/// `reverse` de `leet_convert` (le texte d'origine n'est pas gardé).
pub fn unleet(text: &str) -> String {
    let mut out = text.to_string();
    for (from, to) in [
        ("|<", "k"),
        ("><", "x"),
        ("$", "s"),
        ("(", "c"),
        ("4", "a"),
        ("3", "e"),
        ("1", "i"),
        ("0", "o"),
        ("5", "s"),
        ("7", "t"),
        ("8", "b"),
        ("9", "g"),
        ("2", "z"),
    ] {
        out = out.replace(from, to);
    }
    out
}

#[tauri::command]
pub fn leet_convert(text: String, level: u32, reverse: bool) -> String {
    leet_value(&text, level, reverse)
}

/// Le calcul de `leet_convert`, appelable sans Tauri (tests).
fn leet_value(text: &str, level: u32, reverse: bool) -> String {
    if reverse {
        unleet(text)
    } else {
        Leet::transform(text, level_from_number(level))
    }
}

#[tauri::command]
pub fn rewrite_dictation(
    state: State<'_, AppState>,
    id: String,
    to_leet: bool,
) -> Result<Dictation, String> {
    let id = Uuid::parse_str(&id).map_err(|error| format!("identifiant invalide : {error}"))?;
    let level = level_from_prefs(&state.prefs());
    let mut history = state.history();
    let mut dictation = history
        .items()
        .iter()
        .find(|item| item.id == id)
        .cloned()
        .ok_or_else(|| "dictée introuvable".to_string())?;

    dictation.text = if to_leet {
        Leet::transform(&dictation.raw, level)
    } else {
        dictation.raw.clone()
    };
    dictation.leet = to_leet;
    history.replace(dictation.clone());
    Ok(dictation)
}

// MARK: Réessayer

/// La langue parlée des réglages (`auto`, `fr`, `en`…).
fn language_name(prefs: &Prefs) -> String {
    match prefs.language {
        DictationLanguage::Auto => "auto",
        DictationLanguage::Fr => "fr",
        DictationLanguage::En => "en",
        DictationLanguage::Es => "es",
        DictationLanguage::De => "de",
        DictationLanguage::It => "it",
        DictationLanguage::Pt => "pt",
    }
    .to_string()
}

/// Re-transcrit l'audio gardé d'une dictée ratée, avec le modèle courant.
#[tauri::command]
pub fn retry_dictation(state: State<'_, AppState>, id: String) -> Result<Dictation, String> {
    let id = Uuid::parse_str(&id).map_err(|error| format!("identifiant invalide : {error}"))?;
    let (mut dictation, audio_folder, audio_name) = {
        let history = state.history();
        let dictation = history
            .items()
            .iter()
            .find(|item| item.id == id)
            .cloned()
            .ok_or_else(|| "dictée introuvable".to_string())?;
        let audio_name = dictation
            .audio_file
            .clone()
            .ok_or_else(|| "aucun audio gardé pour cette dictée".to_string())?;
        (dictation, history.audio_folder().to_path_buf(), audio_name)
    };
    let audio = audio_folder.join(&audio_name);
    let samples = plume_core::audio::wav::read_wav_16k_mono(&audio)
        .map_err(|error| format!("audio illisible : {error:#}"))?;

    let prefs = state.prefs().clone();
    let def = models::model(&prefs.model)
        .ok_or_else(|| format!("modèle inconnu : {}", prefs.model))?;
    let engine = Engine::load(
        &models::model_dir(def.id),
        models::family(def),
        4,
        &language_name(&prefs),
    )
    .map_err(|error| format!("moteur : {error:#}"))?;
    let raw = transcribe_dictation(&engine, &samples)
        .map_err(|error| format!("moteur : {error:#}"))?
        .ok_or_else(|| "rien entendu".to_string())?;

    let vocabulary = Vocabulary::new(read_vocabulary());
    let text = plume_core::text::finish(&raw, &vocabulary);
    dictation.raw = raw;
    dictation.text = text;
    dictation.polished = false;
    dictation.pasted = false;
    dictation.failed = false;
    dictation.audio_file = None;
    dictation.leet = false;

    let mut history = state.history();
    history.replace(dictation.clone());
    drop(history);
    let _ = std::fs::remove_file(&audio);
    Ok(dictation)
}

// MARK: Presse-papiers et lissage

#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("presse-papiers : {error}"))?;
    clipboard
        .set_text(text)
        .map_err(|error| format!("presse-papiers : {error}"))
}

/// La phrase fixe du test de lissage.
pub const POLISH_SAMPLE: &str =
    "euh bonjour je voulais te dire que la réunion est décalée";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolishTest {
    pub ok: bool,
    pub ms: u64,
    pub sample: String,
    pub error: Option<String>,
}

/// Appelle le service de lissage une fois, sur la phrase fixe, et rend la
/// latence brute de l'API.
#[tauri::command]
pub fn polish_test(base_url: String, model: String, api_key: String) -> PolishTest {
    polish_test_value(base_url, model, api_key)
}

/// Le calcul de `polish_test`, appelable sans Tauri (tests).
fn polish_test_value(base_url: String, model: String, api_key: String) -> PolishTest {
    let config = plume_core::text::PolishConfig::new(
        base_url,
        model,
        if api_key.trim().is_empty() {
            None
        } else {
            Some(api_key)
        },
    );
    let started = Instant::now();
    let polished = plume_core::text::polish::polish_opt(&config, POLISH_SAMPLE, "fr");
    let ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    match polished {
        Some(sample) => PolishTest {
            ok: true,
            ms,
            sample,
            error: None,
        },
        None => PolishTest {
            ok: false,
            ms,
            sample: POLISH_SAMPLE.to_string(),
            error: Some("lissage impossible (adresse, clé ou service)".into()),
        },
    }
}

// MARK: Capture de touche

/// Arme (ou désarme) la capture de la prochaine touche seule.
///
/// Pendant la capture, `ParakeetTranscriber::capture_active()` fait ignorer le
/// geste de dictée par le chef d'orchestre.
#[tauri::command]
pub fn set_dictation_key_capture(app: AppHandle, on: bool) {
    ParakeetTranscriber::set_capture_active(on);
    CAPTURE_ARMED.store(on, Ordering::SeqCst);
    if on {
        start_capture_listener(app);
    }
}

/// L'écouteur de capture : un seul pour la vie de l'app, armé par le drapeau.
fn start_capture_listener(app: AppHandle) {
    if CAPTURE_LISTENER.set(()).is_err() {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("plume-capture-touche".into())
        .spawn(move || {
            #[derive(Default)]
            struct CaptureState {
                down: Option<(rdev::Key, Instant)>,
                dirty: bool,
            }
            let state = Mutex::new(CaptureState::default());
            let result = rdev::listen(move |event| {
                if !CAPTURE_ARMED.load(Ordering::SeqCst) {
                    *lock(&state) = CaptureState::default();
                    return;
                }
                let now = Instant::now();
                let mut state = lock(&state);
                match event.event_type {
                    rdev::EventType::KeyPress(key) => match state.down {
                        None => {
                            state.down = Some((key, now));
                            state.dirty = false;
                        }
                        Some((down, _)) if down != key => state.dirty = true,
                        Some(_) => {}
                    },
                    rdev::EventType::KeyRelease(key) => {
                        let captured = match state.down.take() {
                            Some((down, at))
                                if down == key
                                    && !state.dirty
                                    && now.duration_since(at) <= Duration::from_millis(800) =>
                            {
                                Some(down)
                            }
                            _ => None,
                        };
                        state.dirty = false;
                        if let Some(key) = captured {
                            let key = key_of(key);
                            if key == Key::Other {
                                // Une lettre : on reste armé, l'utilisateur
                                // choisit une des touches proposées.
                                return;
                            }
                            CAPTURE_ARMED.store(false, Ordering::SeqCst);
                            drop(state);
                            ParakeetTranscriber::set_capture_active(false);
                            let _ = app.emit(
                                "plume://key-captured",
                                KeyCapturedEvent { name: key.name() },
                            );
                        }
                    }
                    rdev::EventType::ButtonPress(_)
                    | rdev::EventType::ButtonRelease(_)
                    | rdev::EventType::Wheel { .. } => state.dirty = true,
                    rdev::EventType::MouseMove { .. } => {}
                }
            });
            if let Err(error) = result {
                log::warn!("capture de touche : {error:?}");
            }
        });
    if let Err(error) = spawned {
        log::warn!("fil de capture : {error}");
    }
}

/// Les touches que les réglages savent nommer.
fn key_of(key: rdev::Key) -> Key {
    match key {
        rdev::Key::Alt => Key::AltLeft,
        rdev::Key::AltGr => Key::AltRight,
        rdev::Key::ControlLeft => Key::ControlLeft,
        rdev::Key::ControlRight => Key::ControlRight,
        rdev::Key::ShiftLeft => Key::ShiftLeft,
        rdev::Key::ShiftRight => Key::ShiftRight,
        rdev::Key::MetaLeft => Key::MetaLeft,
        rdev::Key::MetaRight => Key::MetaRight,
        rdev::Key::F1 => Key::F(1),
        rdev::Key::F2 => Key::F(2),
        rdev::Key::F3 => Key::F(3),
        rdev::Key::F4 => Key::F(4),
        rdev::Key::F5 => Key::F(5),
        rdev::Key::F6 => Key::F(6),
        rdev::Key::F7 => Key::F(7),
        rdev::Key::F8 => Key::F(8),
        rdev::Key::F9 => Key::F(9),
        rdev::Key::F10 => Key::F(10),
        rdev::Key::F11 => Key::F(11),
        rdev::Key::F12 => Key::F(12),
        rdev::Key::F13 => Key::F(13),
        rdev::Key::F14 => Key::F(14),
        rdev::Key::F15 => Key::F(15),
        rdev::Key::F16 => Key::F(16),
        rdev::Key::F17 => Key::F(17),
        rdev::Key::F18 => Key::F(18),
        rdev::Key::F19 => Key::F(19),
        rdev::Key::F20 => Key::F(20),
        rdev::Key::F21 => Key::F(21),
        rdev::Key::F22 => Key::F(22),
        rdev::Key::F23 => Key::F(23),
        rdev::Key::F24 => Key::F(24),
        _ => Key::Other,
    }
}

// MARK: Nouveautés

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsNewItem {
    pub title_key: String,
    pub body_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsNew {
    pub version: String,
    pub items: Vec<WhatsNewItem>,
}

/// Les notes de la dernière version de `WhatsNew.swift` (1.5), rattachées aux
/// clés déjà présentes dans `ui/i18n/en.json`.
const WHATS_NEW_ITEMS: [(&str, &str); 3] = [
    (
        "1337_dictation",
        "dictate_as_usual_then_finish_with_this_shortcut_that_dictation_comes_out_in_1337_finish_with_your_usual_shortcut_for_plain_text",
    ),
    (
        "indicator_style",
        "the_indicator_takes_no_room_in_the_menu_bar_it_draws_on_top_of_it_both_top_styles_glide_in_from_the_top_edge_of_the_screen",
    ),
    ("skip", "use_the_default_settings"),
];

/// La version de l'app (celle de `Cargo.toml`).
pub fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn get_whats_new() -> WhatsNew {
    whats_new_payload()
}

/// Le contenu de `get_whats_new`, appelable sans Tauri (tests).
fn whats_new_payload() -> WhatsNew {
    WhatsNew {
        version: current_version(),
        items: WHATS_NEW_ITEMS
            .iter()
            .map(|(title, body)| WhatsNewItem {
                title_key: (*title).to_string(),
                body_key: (*body).to_string(),
            })
            .collect(),
    }
}

#[tauri::command]
pub fn mark_whats_new_seen(state: State<'_, AppState>) {
    let mut prefs = state.prefs();
    prefs.last_seen_version = Some(current_version());
    if let Err(error) = prefs.save() {
        log::warn!("dernière version vue non enregistrée : {error}");
    }
}

// MARK: Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_leet_ramene_les_sequences_longues_et_les_chiffres() {
        assert_eq!(unleet("54lu7"), "salut");
        assert_eq!(unleet("8onjour"), "bonjour");
        assert_eq!(unleet("|<!x"), "k!x");
        // Le 1337 puis le retour : les lettres reviennent (le `1` redevient
        // `i`, le `l` maquillé est perdu — c'est le prix du dé-maquillage).
        let disguised = Leet::transform("test", Level::Full);
        assert_eq!(unleet(&disguised).to_lowercase(), "test");
        let sentence = unleet(&Leet::transform("salut les amis", Level::Full));
        assert!(!sentence.chars().any(|c| c.is_ascii_digit()), "{sentence}");
    }

    #[test]
    fn leet_convert_uses_the_three_levels() {
        let light = leet_value("test", 1, false);
        assert!(light.contains('7') || light.contains('3'));
        let medium = leet_value("test", 2, false);
        let full = leet_value("test", 3, false);
        assert!(!light.is_empty() && !medium.is_empty() && !full.is_empty());
        assert_eq!(leet_value("54lu7", 2, true), "salut");
    }

    #[test]
    fn whats_new_items_exist_in_both_translations() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/i18n");
        for language in ["en", "fr"] {
            let raw = std::fs::read_to_string(root.join(format!("{language}.json")))
                .expect("fichier de traduction");
            let strings: serde_json::Value = serde_json::from_str(&raw).expect("JSON");
            let object = strings.as_object().expect("objet");
            for (title, body) in WHATS_NEW_ITEMS {
                assert!(object.contains_key(title), "{language} : {title}");
                assert!(object.contains_key(body), "{language} : {body}");
            }
        }
        assert_eq!(WHATS_NEW_ITEMS.len(), 3);
    }

    /// Les deux traductions ont le même jeu de clés, et rien n'y parle plus du
    /// Mac : ce portage est Linux/Windows (« This computer », « Alt gauche »).
    #[test]
    fn les_traductions_ne_parlent_plus_du_mac() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/i18n");
        let mut keys: Vec<Vec<String>> = Vec::new();
        for language in ["en", "fr"] {
            let raw = std::fs::read_to_string(root.join(format!("{language}.json")))
                .expect("fichier de traduction");
            let strings: serde_json::Value = serde_json::from_str(&raw).expect("JSON");
            let object = strings.as_object().expect("objet");
            let mut own: Vec<String> = object.keys().cloned().collect();
            own.sort();
            keys.push(own);
            for (key, value) in object {
                let text = value.as_str().expect("chaîne");
                assert!(
                    !text.contains("Neural Engine"),
                    "{language} : {key} parle encore de Neural Engine"
                );
                for forbidden in ["this Mac", "This Mac", "your Mac", "ce Mac", "votre Mac"] {
                    assert!(
                        !text.contains(forbidden),
                        "{language} : {key} dit encore « {forbidden} »"
                    );
                }
            }
        }
        assert_eq!(keys[0], keys[1], "les clés en/fr diffèrent");
    }

    /// Les libellés de touche sont ceux de Linux/Windows, jamais les symboles
    /// du Mac (`⌥`, `⌘`, « Left ⌥ »).
    #[test]
    fn les_libelles_de_touche_sont_ceux_de_linux() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/i18n");
        for language in ["en", "fr"] {
            let raw = std::fs::read_to_string(root.join(format!("{language}.json")))
                .expect("fichier de traduction");
            let strings: serde_json::Value = serde_json::from_str(&raw).expect("JSON");
            let object = strings.as_object().expect("objet");
            for key in [
                "key_left_alt",
                "key_right_alt",
                "key_left_ctrl",
                "key_right_ctrl",
                "key_super",
                "key_right_super",
            ] {
                let label = object[key].as_str().expect("libellé");
                assert!(!label.is_empty(), "{language} : {key}");
                for symbol in ['⌥', '⌘', '⌃', '⇧'] {
                    assert!(!label.contains(symbol), "{language} : {key} = {label}");
                }
            }
            let alt = object["key_left_alt"].as_str().unwrap();
            assert!(alt.contains("Alt"), "{language} : {alt}");
            let super_label = object["key_super"].as_str().unwrap();
            assert!(
                super_label.contains("Super"),
                "{language} : {super_label}"
            );
        }
    }

    #[test]
    fn the_whats_new_payload_carries_the_cargo_version() {
        let payload = whats_new_payload();
        assert_eq!(payload.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(payload.items.len(), 3);
        assert_eq!(payload.items[0].title_key, "1337_dictation");
        assert!(!payload.items[2].body_key.is_empty());
    }

    #[test]
    fn the_polish_sample_is_the_fixed_sentence() {
        assert_eq!(
            POLISH_SAMPLE,
            "euh bonjour je voulais te dire que la réunion est décalée"
        );
    }

    #[test]
    fn the_polish_test_reports_a_failure_without_network() {
        let result = polish_test_value(
            "http://127.0.0.1:1/v1".into(),
            "modele".into(),
            "cle".into(),
        );
        assert!(!result.ok);
        assert_eq!(result.sample, POLISH_SAMPLE);
        assert!(result.error.is_some());
    }

    #[test]
    fn key_names_match_the_settings_parser() {
        for key in [
            rdev::Key::Alt,
            rdev::Key::AltGr,
            rdev::Key::ControlRight,
            rdev::Key::MetaRight,
            rdev::Key::F9,
        ] {
            let named = key_of(key);
            assert_ne!(named, Key::Other);
            assert_eq!(Key::parse(&named.name()), Some(named));
        }
        assert_eq!(key_of(rdev::Key::KeyA), Key::Other);
    }

    #[test]
    fn mic_names_are_never_empty() {
        // Sans micro (conteneur), la liste est vide ; avec, aucun nom vide.
        for name in mic_names() {
            assert!(!name.trim().is_empty());
        }
    }

    #[test]
    fn engine_status_follows_the_chosen_model() {
        let mut prefs = Prefs::default();
        prefs.model = "parakeet-v3".into();
        let status = current_engine_status(&prefs);
        assert_eq!(status.model_id, "parakeet-v3");
        // Le modèle de test n'est pas installé dans le dossier de données.
        assert!(matches!(
            status.state.as_str(),
            "missing" | "ready" | "loaded" | "downloading"
        ));
        prefs.model = "inconnu".into();
        assert_eq!(current_engine_status(&prefs).state, "missing");
    }

    #[test]
    fn a_model_archive_root_is_found_by_its_files() {
        let root = std::env::temp_dir().join(format!("plume-archive-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let inner = root.join("sherpa-onnx-whisper-base");
        std::fs::create_dir_all(inner.join("test_wavs")).unwrap();
        std::fs::write(inner.join("base-encoder.int8.onnx"), b"x").unwrap();
        std::fs::write(inner.join("base-tokens.txt"), b"x").unwrap();

        let keep = &["base-encoder.int8.onnx", "base-tokens.txt"];
        assert_eq!(find_archive_root(&root, keep), Some(inner.clone()));
        assert_eq!(find_archive_root(&inner, keep), Some(inner.clone()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sha256_is_the_lowercase_hex_digest_of_the_file() {
        let path = std::env::temp_dir().join(format!("plume-sha-{}", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        let digest = sha256_file(&path).expect("sha256");
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(digest.len(), 64);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_bad_sha256_is_refused() {
        let path = std::env::temp_dir().join(format!("plume-sha-bad-{}", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        assert!(verify_sha256(&path, "0".repeat(64).as_str()).is_err());
        assert!(verify_sha256(
            &path,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        )
        .is_ok());
        std::fs::remove_file(&path).ok();
    }

    /// Le vrai bout en bout : télécharge `whisper-base` dans un dossier de
    /// données temporaire, vérifie le sha256, charge le modèle et transcrit
    /// `/wav/court.wav` ; puis refait la même chose avec Parakeet par le
    /// `Engine` généralisé. Réseau et modèles de test requis.
    #[test]
    #[ignore = "réseau + ~207 Mo de whisper-base + modèles de test du conteneur"]
    fn e2e_whisper_base_download_and_both_engines_transcribe() {
        let dir = std::env::temp_dir().join(format!("plume-e2e-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("dossier temporaire");
        // `paths` lit la variable à chaque appel : on la pose avant tout.
        std::env::set_var(paths::DATA_DIR_ENV, &dir);

        let def = models::model("whisper-base").expect("catalogue");
        let cancel = AtomicBool::new(false);
        let part = paths::models_dir().join(format!(".{}.part", def.id));
        std::fs::create_dir_all(paths::models_dir()).expect("dossier des modèles");
        fetch_archive(def, &part, &cancel, &mut |_, _| {})
            .unwrap_or_else(|error| panic!("téléchargement : {error}"));
        verify_sha256(&part, def.sha256).unwrap_or_else(|error| panic!("sha256 : {error}"));
        install_archive(def, &part, &cancel).unwrap_or_else(|error| panic!("installation : {error}"));
        assert!(models::is_installed(def), "fichiers du modèle installés");

        let wav_dir = std::env::var("PLUME_TEST_WAV").unwrap_or_else(|_| "/wav".to_string());
        let samples = plume_core::audio::wav::read_wav_16k_mono(
            Path::new(&wav_dir).join("court.wav"),
        )
        .expect("court.wav lisible");
        assert!(!samples.is_empty());

        let whisper = Engine::load(
            &models::model_dir("whisper-base"),
            models::family(def),
            4,
            "fr",
        )
        .expect("whisper-base chargé");
        let text = transcribe_dictation(&whisper, &samples)
            .expect("transcription whisper")
            .unwrap_or_default();
        assert!(!text.trim().is_empty(), "whisper-base n'a rien transcrit");
        println!("whisper-base : {text}");

        let parakeet_dir = std::env::var("PLUME_TEST_MODEL")
            .unwrap_or_else(|_| "/models/parakeet-v3".to_string());
        let parakeet = Engine::load(
            Path::new(&parakeet_dir),
            plume_core::engine::Family::Parakeet,
            4,
            "fr",
        )
        .expect("parakeet chargé");
        let text = transcribe_dictation(&parakeet, &samples)
            .expect("transcription parakeet")
            .unwrap_or_default();
        assert!(!text.trim().is_empty(), "parakeet n'a rien transcrit");
        println!("parakeet : {text}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
