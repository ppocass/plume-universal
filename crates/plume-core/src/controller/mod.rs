//! Le chef d'orchestre de la dictée — port de `DictationController.swift`.
//!
//! Un appui : on enregistre ; un appui : on transcrit, on nettoie, on colle ;
//! deux appuis rapides : annulé, rien n'est écrit. Le modèle se charge
//! **pendant** qu'on parle (au premier appui), et reste chargé
//! [`Settings::idle_release_secs`] secondes après la dernière dictée.
//!
//! Le geste lui-même (fenêtre du double appui, 1337, garde-fou des 10 min) est
//! tranché par [`crate::input::gesture`] : ici on ne fait que suivre le cycle de
//! vie et enchaîner les étapes, dans l'ordre de `DictationController.process` :
//! transcription → barrière anti-silence (dans le moteur) → vocabulaire et
//! commandes vocales → lissage → 1337 → collage.
//!
//! **Tous les effets de bord passent par trois petits traits** ([`Recorder`],
//! [`Transcriber`], [`Paster`]) : le flux complet se teste sans micro, sans
//! modèle et sans presse-papiers.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::input::gesture::{GestureConfig, Key, Output};
use crate::text::{Level, Leet, PolishConfig, VocabEntry, Vocabulary};

mod real;

pub use real::{ParakeetTranscriber, SystemPaster, SystemRecorder, PASTE_LOG_ENV};

/// Fils d'exécution du moteur : 4 est le bon réglage mesuré, 8 n'apporte rien.
pub const ENGINE_THREADS: i32 = 4;

/// `Prefs.idleSeconds` : le moteur s'éteint après 35 s sans dictée.
pub const IDLE_RELEASE_SECS: u64 = 35;

/// ~20 événements de niveau par seconde pendant l'enregistrement (les 13
/// barres de la pastille macOS).
pub const LEVEL_INTERVAL: Duration = Duration::from_millis(50);

/// Les réglages du chef d'orchestre. **Structure simple, sans persistance** :
/// c'est un autre module qui lit et écrit les préférences.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// La touche de dictée (`Shortcut` de macOS : ⌥ par défaut).
    pub key: Key,
    /// Le modificateur 1337 tenu au moment du Stop (`leetShortcut`).
    pub leet_key: Option<Key>,
    /// Le dossier du modèle Parakeet (les quatre fichiers `.onnx`/`tokens.txt`).
    pub model_dir: PathBuf,
    /// Fils d'exécution du moteur (4).
    pub threads: i32,
    /// Secondes sans dictée avant de relâcher le moteur (35 par défaut).
    pub idle_release_secs: u64,
    /// Le lissage (`polishEnabled` + service/modèle). `None` : pas de lissage.
    pub polish: Option<PolishConfig>,
    /// Le fichier `vocabulaire.json` (tableau JSON de `VocabEntry`, comme
    /// `VocabularyStore`), relu à chaque dictée.
    pub vocabulary: Option<PathBuf>,
    /// La langue parlée : `auto`, `fr`, `en`… (`Prefs.effectiveLanguage`).
    pub lang: String,
    /// Niveau du 1337 quand le geste 1337 termine la dictée
    /// (`Prefs.leetLevel`, `.medium` par défaut).
    pub leet_level: Level,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            key: Key::AltLeft,
            leet_key: Some(Key::MetaLeft),
            model_dir: PathBuf::new(),
            threads: ENGINE_THREADS,
            idle_release_secs: IDLE_RELEASE_SECS,
            polish: None,
            vocabulary: None,
            lang: "auto".to_string(),
            leet_level: Level::Medium,
        }
    }
}

impl Settings {
    /// Les réglages par défaut d'un modèle donné.
    pub fn new(model_dir: impl Into<PathBuf>) -> Self {
        Self {
            model_dir: model_dir.into(),
            ..Self::default()
        }
    }

    /// Le geste de dictée correspondant.
    pub fn gesture_config(&self) -> GestureConfig {
        GestureConfig {
            key: self.key,
            leet_key: self.leet_key,
            double_tap_window: crate::input::gesture::DOUBLE_TAP_WINDOW,
        }
    }
}

/// Ce que montre la pastille. `Done { pasted }` : `false` quand le texte n'a
/// pas pu partir au curseur (il reste dans l'événement [`Event::Text`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Recording,
    Processing,
    Done { pasted: bool },
    Cancelled,
    /// La barrière n'a rien laissé passer (silence, bruit, clic), avec sa
    /// raison ; rien n'est collé.
    Rejected(String),
    Error(String),
}

/// Ce que le chef d'orchestre raconte à l'extérieur (pastille, `plume-cli`).
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Phase(Phase),
    /// Le niveau du micro, 0…1, ~20 fois par seconde pendant l'enregistrement.
    Level(f32),
    /// Le texte final, juste avant le collage.
    Text(String),
}

/// Le micro : ouvert au premier appui, fermé au second.
pub trait Recorder {
    fn start(&mut self) -> Result<()>;
    /// Ferme le micro et rend l'enregistrement (16 kHz mono).
    fn stop(&mut self) -> Vec<f32>;
    /// Le niveau 0…1 du moment.
    fn level(&self) -> f32;
}

/// Le résultat d'une transcription.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Transcribed {
    /// La barrière a laissé passer de la parole : voici le texte.
    Text(String),
    /// Rien à transcrire (silence, bruit, clic, trop court), avec la raison
    /// donnée par la barrière.
    Rejected(String),
}

/// Le moteur Parakeet : chargé en tâche de fond, réutilisé d'une dictée à
/// l'autre, relâché à la mise en veille.
///
/// `load` et `transcribe` prennent `&self` : le chargement vit dans un fil à
/// part (il dure quelques secondes) pendant que l'enregistrement continue.
pub trait Transcriber {
    /// Vrai quand un moteur est prêt **ou en cours de chargement** : on ne
    /// lance pas deux chargements pour une dictée.
    fn is_loaded(&self) -> bool;
    fn load(&self) -> Result<()>;
    fn transcribe(&self, samples: &[f32]) -> Result<Transcribed>;
    /// Relâche le moteur (mémoire rendue au système).
    fn release(&self);
}

/// Le collage au curseur.
pub trait Paster {
    fn paste(&self, text: &str) -> Result<()>;
}

/// Le chef d'orchestre. Il possède le canal des gestes, le micro, le moteur et
/// le collage ; il envoie chaque changement de phase dans `events`.
pub struct Controller<R: Recorder, T: Transcriber + Send + Sync + 'static, P: Paster> {
    settings: Settings,
    events: Sender<Event>,
    recorder: R,
    transcriber: Arc<T>,
    paster: P,
    phase: Phase,
    /// Depuis combien de temps on enregistre (journal).
    recording_since: Option<Instant>,
    /// Quand relâcher le moteur, s'il tourne encore.
    idle_deadline: Option<Instant>,
}

impl<R, T, P> Controller<R, T, P>
where
    R: Recorder,
    T: Transcriber + Send + Sync + 'static,
    P: Paster,
{
    pub fn new(
        settings: Settings,
        events: Sender<Event>,
        recorder: R,
        transcriber: T,
        paster: P,
    ) -> Self {
        Self {
            settings,
            events,
            recorder,
            transcriber: Arc::new(transcriber),
            paster,
            phase: Phase::Idle,
            recording_since: None,
            idle_deadline: None,
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// Le geste configuré (`key`, `leet_key`, fenêtre du double appui).
    pub fn gesture_config(&self) -> GestureConfig {
        self.settings.gesture_config()
    }

    /// L'écoute globale et la boucle de vie : chaque geste, plus le battement
    /// qui envoie les niveaux et relâche le moteur.
    ///
    /// Rend la main quand l'écoute s'arrête (session X fermée, écoute
    /// impossible).
    pub fn run(&mut self) -> Result<()> {
        let gestures = crate::input::listener::start(self.gesture_config())?;
        loop {
            match gestures.recv_timeout(self.next_timeout()) {
                Ok(output) => self.handle(output),
                Err(RecvTimeoutError::Timeout) => self.tick(),
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }
        }
    }

    /// Un geste de la machine à états.
    ///
    /// Pendant le traitement, un appui ne fait rien : la dictée en cours va
    /// être collée, on ne veut pas en démarrer une autre par erreur.
    pub fn handle(&mut self, output: Output) {
        if self.phase == Phase::Processing {
            return;
        }
        match output {
            Output::Start => self.start(),
            Output::Stop { leet } => self.stop(leet),
            Output::Cancel => self.cancel(),
        }
    }

    /// Le battement : niveau du micro pendant l'enregistrement, mise en veille
    /// du moteur après une dictée.
    pub fn tick(&mut self) {
        self.tick_at(Instant::now());
    }

    /// `tick` à une heure donnée (les tests n'attendent pas 35 s).
    pub fn tick_at(&mut self, now: Instant) {
        if self.phase == Phase::Recording {
            let _ = self.events.send(Event::Level(self.recorder.level()));
            return;
        }
        if let Some(deadline) = self.idle_deadline {
            if now >= deadline {
                self.idle_deadline = None;
                log::info!(
                    "moteur mis en veille ({} s sans dictée)",
                    self.settings.idle_release_secs
                );
                self.transcriber.release();
            }
        }
    }

    // MARK: Enregistrement

    fn start(&mut self) {
        if matches!(self.phase, Phase::Recording | Phase::Processing) {
            return;
        }
        if let Err(error) = self.recorder.start() {
            log::info!("micro : {error:#}");
            self.emit_phase(Phase::Error(format!("micro indisponible : {error:#}")));
            return;
        }
        self.recording_since = Some(Instant::now());
        self.idle_deadline = None;
        self.emit_phase(Phase::Recording);

        // Le modèle se charge PENDANT qu'on parle (macOS :
        // `SpeechEngine.engine(for:).ensureRunning()`).
        self.load_engine_in_background();
    }

    fn stop(&mut self, leet: bool) {
        if self.phase != Phase::Recording {
            return;
        }
        let samples = self.recorder.stop();
        let seconds = self
            .recording_since
            .take()
            .map(|started| started.elapsed().as_secs_f64())
            .unwrap_or_else(|| samples.len() as f64 / crate::audio::gate::SAMPLE_RATE as f64);
        log::info!("fin de dictée ({seconds:.1} s), traitement…");
        self.emit_phase(Phase::Processing);
        if let Err(error) = self.process(&samples, leet) {
            log::info!("traitement : {error:#}");
            self.emit_phase(Phase::Error(format!("{error:#}")));
        }
        self.schedule_idle_release();
    }

    fn cancel(&mut self) {
        if self.phase != Phase::Recording {
            return;
        }
        // L'audio est jeté avec l'enregistrement : rien n'est transcrit.
        let _ = self.recorder.stop();
        self.recording_since = None;
        log::info!("dictée annulée (double appui)");
        self.emit_phase(Phase::Cancelled);
        self.schedule_idle_release();
    }

    // MARK: Traitement

    /// Transcrit, nettoie, maquille et colle — l'ordre exact de
    /// `DictationController.process`.
    fn process(&mut self, samples: &[f32], leet: bool) -> Result<()> {
        let raw = match self.transcriber.transcribe(samples)? {
            Transcribed::Text(text) => text,
            Transcribed::Rejected(reason) => {
                log::info!("rien à transcrire ({reason})");
                self.emit_phase(Phase::Rejected(reason));
                return Ok(());
            }
        };

        // Le vocabulaire est relu à chaque dictée (comme le `snapshot` de
        // `VocabularyStore` au début du traitement).
        let vocabulary = self.vocabulary();
        let mut text = crate::text::finish(&raw, &vocabulary);
        if text.trim().is_empty() {
            self.emit_phase(Phase::Rejected("rien entendu".into()));
            return Ok(());
        }

        // Le lissage ne rend jamais d'erreur : au moindre problème (hors ligne,
        // clé absente, délai, réponse bizarre), il rend le texte brut.
        if let Some(polish) = &self.settings.polish {
            let polished = crate::text::polish(polish, &text, &self.settings.lang);
            text = vocabulary.apply(&polished);
        }

        // Le maquillage 1337 se fait APRÈS tout le reste (vocabulaire,
        // commandes vocales, lissage) : c'est le texte final qu'on habille.
        if leet {
            text = Leet::transform(&text, self.settings.leet_level);
            log::info!("dictée 1337 ({})", self.settings.leet_level.as_str());
        }

        let _ = self.events.send(Event::Text(text.clone()));
        match self.paster.paste(&text) {
            Ok(()) => self.emit_phase(Phase::Done { pasted: true }),
            Err(error) => self.emit_phase(Phase::Error(format!("collage impossible : {error:#}"))),
        }
        Ok(())
    }

    /// Le vocabulaire du fichier de réglages (`vocabulaire.json` : un tableau
    /// de `VocabEntry`, comme Swift ; l'objet `{"entries": […]}` est toléré).
    fn vocabulary(&self) -> Vocabulary {
        let Some(path) = &self.settings.vocabulary else {
            return Vocabulary::default();
        };
        match std::fs::read_to_string(path) {
            Ok(raw) => match serde_json::from_str::<Vec<VocabEntry>>(&raw) {
                Ok(entries) => Vocabulary::new(entries),
                Err(array_error) => match serde_json::from_str::<Vocabulary>(&raw) {
                    Ok(vocabulary) => vocabulary,
                    Err(_) => {
                        log::warn!("vocabulaire illisible ({}) : {array_error}", path.display());
                        Vocabulary::default()
                    }
                },
            },
            Err(error) => {
                log::warn!("vocabulaire introuvable ({}) : {error}", path.display());
                Vocabulary::default()
            }
        }
    }

    // MARK: Moteur

    /// Lance le chargement s'il n'a pas déjà eu lieu — dans un fil, pour ne pas
    /// retarder le début de l'enregistrement.
    fn load_engine_in_background(&self) {
        if self.transcriber.is_loaded() {
            return;
        }
        let transcriber = Arc::clone(&self.transcriber);
        let spawned = thread::Builder::new()
            .name("plume-moteur".into())
            .spawn(move || {
                if let Err(error) = transcriber.load() {
                    log::info!("chargement du moteur : {error:#}");
                }
            });
        if let Err(error) = spawned {
            log::error!("fil du moteur : {error}");
        }
    }

    /// Relâche le moteur après `idle_release_secs` sans dictée (la montre est
    /// tenue par `tick`, pas par un fil : les tests peuvent la faire avancer).
    fn schedule_idle_release(&mut self) {
        self.idle_deadline =
            Some(Instant::now() + Duration::from_secs(self.settings.idle_release_secs));
    }

    /// Le délai d'attente du prochain battement.
    fn next_timeout(&self) -> Duration {
        if self.phase == Phase::Recording {
            return LEVEL_INTERVAL;
        }
        match self.idle_deadline {
            Some(deadline) => deadline.saturating_duration_since(Instant::now()),
            None => Duration::from_secs(3_600),
        }
    }

    fn emit_phase(&mut self, phase: Phase) {
        self.phase = phase.clone();
        let _ = self.events.send(Event::Phase(phase));
    }
}

/// Un canal d'événements et son récepteur, pour les appelants qui lisent les
/// phases ailleurs (la pastille, `plume-cli run`).
pub fn channel() -> (Sender<Event>, Receiver<Event>) {
    mpsc::channel()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    const SAMPLES: usize = 16_000;

    // MARK: Faux micro, faux moteur, faux collage

    #[derive(Default)]
    struct RecorderState {
        samples: Vec<f32>,
        level: f32,
        starts: usize,
        stops: usize,
        fail: bool,
    }

    #[derive(Clone)]
    struct FakeRecorder {
        state: Arc<Mutex<RecorderState>>,
    }

    impl FakeRecorder {
        fn new() -> Self {
            let recorder = Self {
                state: Arc::new(Mutex::new(RecorderState::default())),
            };
            recorder.state.lock().unwrap().samples = vec![0.3; SAMPLES];
            recorder
        }

        fn at_level(level: f32) -> Self {
            let recorder = Self::new();
            recorder.state.lock().unwrap().level = level;
            recorder
        }

        fn failing() -> Self {
            let recorder = Self::new();
            recorder.state.lock().unwrap().fail = true;
            recorder
        }

        fn starts(&self) -> usize {
            self.state.lock().unwrap().starts
        }

        fn stops(&self) -> usize {
            self.state.lock().unwrap().stops
        }
    }

    impl Recorder for FakeRecorder {
        fn start(&mut self) -> Result<()> {
            let mut state = self.state.lock().unwrap();
            if state.fail {
                anyhow::bail!("micro refusé");
            }
            state.starts += 1;
            Ok(())
        }

        fn stop(&mut self) -> Vec<f32> {
            let mut state = self.state.lock().unwrap();
            state.stops += 1;
            state.samples.clone()
        }

        fn level(&self) -> f32 {
            self.state.lock().unwrap().level
        }
    }

    #[derive(Default)]
    struct TranscriberState {
        loads: usize,
        releases: usize,
        loaded: bool,
    }

    #[derive(Clone)]
    struct FakeTranscriber {
        state: Arc<Mutex<TranscriberState>>,
        next: Arc<Mutex<Transcribed>>,
        error: Arc<Mutex<Option<String>>>,
    }

    impl FakeTranscriber {
        fn new(text: &str) -> Self {
            Self {
                state: Arc::new(Mutex::new(TranscriberState::default())),
                next: Arc::new(Mutex::new(Transcribed::Text(text.to_string()))),
                error: Arc::new(Mutex::new(None)),
            }
        }

        fn rejected(reason: &str) -> Self {
            let transcriber = Self::new("");
            *transcriber.next.lock().unwrap() = Transcribed::Rejected(reason.to_string());
            transcriber
        }

        fn set_text(&self, text: &str) {
            *self.next.lock().unwrap() = Transcribed::Text(text.to_string());
        }

        fn loads(&self) -> usize {
            self.state.lock().unwrap().loads
        }

        fn releases(&self) -> usize {
            self.state.lock().unwrap().releases
        }
    }

    impl Transcriber for FakeTranscriber {
        fn is_loaded(&self) -> bool {
            self.state.lock().unwrap().loaded
        }

        fn load(&self) -> Result<()> {
            if self.state.lock().unwrap().loaded {
                return Ok(());
            }
            let mut state = self.state.lock().unwrap();
            state.loads += 1;
            state.loaded = true;
            Ok(())
        }

        fn transcribe(&self, _samples: &[f32]) -> Result<Transcribed> {
            if let Some(error) = self.error.lock().unwrap().clone() {
                anyhow::bail!("{error}");
            }
            Ok(self.next.lock().unwrap().clone())
        }

        fn release(&self) {
            let mut state = self.state.lock().unwrap();
            state.releases += 1;
            state.loaded = false;
        }
    }

    #[derive(Clone, Default)]
    struct FakePaster {
        pasted: Arc<Mutex<Vec<String>>>,
        fail: bool,
    }

    impl FakePaster {
        fn failing() -> Self {
            Self {
                fail: true,
                ..Self::default()
            }
        }

        fn pasted(&self) -> Vec<String> {
            self.pasted.lock().unwrap().clone()
        }
    }

    impl Paster for FakePaster {
        fn paste(&self, text: &str) -> Result<()> {
            if self.fail {
                anyhow::bail!("collage refusé");
            }
            self.pasted.lock().unwrap().push(text.to_string());
            Ok(())
        }
    }

    // MARK: Banc d'essai

    fn drain(events: &Receiver<Event>) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(event) = events.try_recv() {
            out.push(event);
        }
        out
    }

    struct Bench {
        controller: Controller<FakeRecorder, FakeTranscriber, FakePaster>,
        events: Receiver<Event>,
        recorder: FakeRecorder,
        transcriber: FakeTranscriber,
        paster: FakePaster,
    }

    impl Bench {
        fn new() -> Self {
            Self::with_settings(Settings::default(), FakeTranscriber::new("Salut, je teste."))
        }

        fn with_settings(settings: Settings, transcriber: FakeTranscriber) -> Self {
            Self::with_everything(settings, FakeRecorder::new(), transcriber, FakePaster::default())
        }

        fn with_everything(
            settings: Settings,
            recorder: FakeRecorder,
            transcriber: FakeTranscriber,
            paster: FakePaster,
        ) -> Self {
            let (sender, events) = channel();
            let controller = Controller::new(
                settings,
                sender,
                recorder.clone(),
                transcriber.clone(),
                paster.clone(),
            );
            Self {
                controller,
                events,
                recorder,
                transcriber,
                paster,
            }
        }

        /// Attend le chargement de fond (`load` tourne dans un fil).
        fn wait_for_loads(&self, wanted: usize) {
            for _ in 0..400 {
                if self.transcriber.loads() >= wanted {
                    return;
                }
                thread::sleep(Duration::from_millis(5));
            }
            panic!(
                "le moteur n'a pas été chargé ({} chargements)",
                self.transcriber.loads()
            );
        }

        fn events(&mut self) -> Vec<Event> {
            drain(&self.events)
        }
    }

    // MARK: Les tests

    #[test]
    fn a_dictation_pastes_the_finished_text() {
        let mut bench = Bench::new();

        bench.controller.handle(Output::Start);
        assert_eq!(bench.controller.phase(), &Phase::Recording);
        bench.wait_for_loads(1);
        bench.controller.handle(Output::Stop { leet: false });

        assert_eq!(bench.controller.phase(), &Phase::Done { pasted: true });
        assert_eq!(bench.paster.pasted(), vec!["Salut, je teste.".to_string()]);
        assert_eq!(bench.recorder.starts(), 1);
        assert_eq!(bench.recorder.stops(), 1);

        let events = bench.events();
        assert!(events.contains(&Event::Phase(Phase::Recording)));
        assert!(events.contains(&Event::Phase(Phase::Processing)));
        assert!(events.contains(&Event::Text("Salut, je teste.".to_string())));
        assert!(events.contains(&Event::Phase(Phase::Done { pasted: true })));
    }

    #[test]
    fn cancelling_pastes_nothing() {
        let mut bench = Bench::new();
        bench.controller.handle(Output::Start);
        bench.wait_for_loads(1);
        bench.controller.handle(Output::Cancel);

        assert_eq!(bench.controller.phase(), &Phase::Cancelled);
        assert!(bench.paster.pasted().is_empty());
        assert_eq!(bench.recorder.stops(), 1);
        assert!(!bench
            .events()
            .iter()
            .any(|event| matches!(event, Event::Text(_))));
    }

    #[test]
    fn rejected_audio_is_rejected_and_nothing_is_pasted() {
        let mut bench = Bench::with_settings(
            Settings::default(),
            FakeTranscriber::rejected("pas de parole"),
        );
        bench.controller.handle(Output::Start);
        bench.wait_for_loads(1);
        bench.controller.handle(Output::Stop { leet: false });

        assert_eq!(
            bench.controller.phase(),
            &Phase::Rejected("pas de parole".to_string())
        );
        assert!(bench.paster.pasted().is_empty());
        assert!(!bench
            .events()
            .iter()
            .any(|event| matches!(event, Event::Text(_))));
    }

    #[test]
    fn a_leet_stop_disguises_the_text() {
        let mut settings = Settings::default();
        settings.leet_level = Level::Full;
        let mut bench = Bench::with_settings(settings, FakeTranscriber::new("Salut les amis"));

        bench.controller.handle(Output::Start);
        bench.wait_for_loads(1);
        bench.controller.handle(Output::Stop { leet: true });

        let expected = Leet::transform("Salut les amis", Level::Full);
        assert_ne!(expected, "Salut les amis");
        assert_eq!(bench.paster.pasted(), vec![expected.clone()]);
        assert!(bench.events().contains(&Event::Text(expected)));
        // Le 1337 par défaut est celui de macOS (`.medium`).
        assert_eq!(Settings::default().leet_level, Level::Medium);
    }

    #[test]
    fn a_polish_failure_falls_back_to_the_raw_text() {
        let mut settings = Settings::default();
        // Adresse locale autorisée mais personne n'écoute : connexion refusée.
        settings.polish = Some(PolishConfig::new(
            "http://127.0.0.1:1/v1",
            "modele",
            Some("cle".to_string()),
        ));
        let mut bench = Bench::with_settings(settings, FakeTranscriber::new("Salut, je teste."));

        bench.controller.handle(Output::Start);
        bench.wait_for_loads(1);
        bench.controller.handle(Output::Stop { leet: false });

        assert_eq!(bench.paster.pasted(), vec!["Salut, je teste.".to_string()]);
        assert_eq!(bench.controller.phase(), &Phase::Done { pasted: true });
    }

    #[test]
    fn the_engine_is_loaded_once_across_two_dictations() {
        let mut bench = Bench::new();

        bench.controller.handle(Output::Start);
        bench.wait_for_loads(1);
        bench.controller.handle(Output::Stop { leet: false });

        bench.transcriber.set_text("Deuxième dictée.");
        bench.controller.handle(Output::Start);
        bench.controller.handle(Output::Stop { leet: false });

        assert_eq!(bench.transcriber.loads(), 1);
        assert_eq!(bench.transcriber.releases(), 0);
        assert_eq!(
            bench.paster.pasted(),
            vec![
                "Salut, je teste.".to_string(),
                "Deuxième dictée.".to_string()
            ]
        );
    }

    #[test]
    fn the_engine_is_released_when_idle() {
        let mut settings = Settings::default();
        settings.idle_release_secs = 0;
        let mut bench = Bench::with_settings(settings, FakeTranscriber::new("Salut."));

        bench.controller.handle(Output::Start);
        bench.wait_for_loads(1);
        bench.controller.handle(Output::Stop { leet: false });
        assert_eq!(bench.transcriber.releases(), 0);

        bench.controller.tick();
        assert_eq!(bench.transcriber.releases(), 1);
        assert!(!bench.transcriber.is_loaded());

        // La dictée suivante recharge le modèle, une seule fois.
        bench.controller.handle(Output::Start);
        bench.wait_for_loads(2);
        assert_eq!(bench.transcriber.loads(), 2);
    }

    #[test]
    fn the_level_is_emitted_while_recording_only() {
        let mut bench = Bench::with_everything(
            Settings::default(),
            FakeRecorder::at_level(0.42),
            FakeTranscriber::new("Salut."),
            FakePaster::default(),
        );

        bench.controller.handle(Output::Start);
        bench.controller.tick();
        assert_eq!(
            bench.events(),
            vec![Event::Phase(Phase::Recording), Event::Level(0.42)]
        );

        bench.controller.handle(Output::Stop { leet: false });
        bench.controller.tick();
        assert!(!bench
            .events()
            .iter()
            .any(|event| matches!(event, Event::Level(_))));
    }

    #[test]
    fn a_second_start_while_recording_does_nothing() {
        let mut bench = Bench::new();
        bench.controller.handle(Output::Start);
        bench.wait_for_loads(1);
        bench.controller.handle(Output::Start);

        assert_eq!(bench.recorder.starts(), 1);
        assert_eq!(bench.transcriber.loads(), 1);
        assert_eq!(bench.controller.phase(), &Phase::Recording);
    }

    #[test]
    fn a_stop_without_recording_does_nothing() {
        let mut bench = Bench::new();
        bench.controller.handle(Output::Stop { leet: false });
        bench.controller.handle(Output::Cancel);

        assert_eq!(bench.controller.phase(), &Phase::Idle);
        assert_eq!(bench.recorder.stops(), 0);
        assert!(bench.paster.pasted().is_empty());
    }

    #[test]
    fn a_microphone_failure_ends_in_error_and_pastes_nothing() {
        let mut bench = Bench::with_everything(
            Settings::default(),
            FakeRecorder::failing(),
            FakeTranscriber::new("Salut."),
            FakePaster::default(),
        );

        bench.controller.handle(Output::Start);

        match bench.controller.phase() {
            Phase::Error(message) => assert!(message.contains("micro"), "{message}"),
            other => panic!("attendu Error, obtenu {other:?}"),
        }
        assert_eq!(bench.recorder.starts(), 0);
        assert!(bench.paster.pasted().is_empty());
        assert_eq!(bench.transcriber.loads(), 0);
    }

    #[test]
    fn the_vocabulary_file_is_applied() {
        let dir = std::env::temp_dir().join("plume-controller-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("vocabulaire.json");
        std::fs::write(
            &path,
            r#"[{"id":"1","word":"Plume","variants":["plum"],"addedAt":""}]"#,
        )
        .unwrap();

        let mut settings = Settings::default();
        settings.vocabulary = Some(path.clone());
        let mut bench = Bench::with_settings(settings, FakeTranscriber::new("ouvre plum"));

        bench.controller.handle(Output::Start);
        bench.wait_for_loads(1);
        bench.controller.handle(Output::Stop { leet: false });

        assert_eq!(bench.paster.pasted(), vec!["ouvre Plume".to_string()]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_paste_failure_ends_in_error() {
        let mut bench = Bench::with_everything(
            Settings::default(),
            FakeRecorder::new(),
            FakeTranscriber::new("Salut."),
            FakePaster::failing(),
        );

        bench.controller.handle(Output::Start);
        bench.wait_for_loads(1);
        bench.controller.handle(Output::Stop { leet: false });

        match bench.controller.phase() {
            Phase::Error(message) => assert!(message.contains("collage"), "{message}"),
            other => panic!("attendu Error, obtenu {other:?}"),
        }
        // Le texte a quand même été annoncé.
        assert!(bench
            .events()
            .iter()
            .any(|event| matches!(event, Event::Text(_))));
    }
}
