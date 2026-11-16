//! La machine à états du geste de dictée — le cœur de `HotkeyMonitor.swift` et
//! des gestes de `DictationController.swift`, sans aucune E/S.
//!
//! On lui donne des événements clavier horodatés, elle rend `Start`, `Stop` ou
//! `Cancel`. Rien n'est bloqué, rien n'est modifié : c'est l'appelant qui décide
//! quoi faire de la sortie (ouvrir le micro, coller, annuler).
//!
//! Les règles viennent de macOS, à l'identique :
//!
//! - **Touche modificatrice** (Option gauche…) : un « appui » ne compte que si
//!   la touche est enfoncée puis relâchée **seule** — aucune autre touche, aucun
//!   clic, aucun autre modificateur entre les deux — et en moins de 0,8 s.
//!   Option sert à taper `{ | …` et aux raccourcis : ⌥+lettre ou ⌥-clic ne
//!   déclenchent donc jamais la dictée.
//!
//! - **Un appui** quand rien n'est en cours : `Start`. **Un appui** pendant
//!   l'enregistrement : `Stop` — mais pas tout de suite, on attend la fenêtre du
//!   double appui pour voir si un second appui (= annuler) arrive.
//!
//! - **Deux appuis rapides** d'affilée pendant l'enregistrement (le premier
//!   arme le Stop, le second l'annule) : `Cancel`, rien n'est écrit.
//!
//! - **Le 1337 se choisit à la fin** : c'est le geste de *fin* qui décide du
//!   style. `Stop { leet: true }` quand le modificateur 1337 est tenu au moment
//!   du Stop (macOS : ⌘+⌥ ; ici réglable, Super par défaut).
//!
//! Deux différences assumées avec macOS :
//!
//! - macOS voit des *drapeaux* (`flagsChanged`) et peut donc lire l'état de
//!   toutes les touches à chaque événement ; ici on ne voit que des appuis et
//!   relâchements, d'où le registre `held`.
//! - Le couple ⌘+⌥ de macOS est un geste à part (`handleLeetTap`), qui déclenche
//!   un appui 1337 indépendant de la touche de dictée. Cette version suit la
//!   consigne du portage : le 1337 est **le modificateur 1337 tenu au moment du
//!   Stop**, ce qui donne le même geste à l'usage (⌘+⌥ puis on relâche ⌥).

use std::collections::HashSet;
use std::time::{Duration, Instant};

/// `HotkeyMonitor.maxTapDuration` : au-delà, ce n'était pas un appui mais un
/// maintien (on réfléchit, on regarde un raccourci…) — ignoré.
pub const MAX_TAP_HOLD: Duration = Duration::from_millis(800);

/// `DictationController.doubleTapWindow` : le temps laissé au second appui pour
/// annuler, une fois le Stop armé.
pub const DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(300);

/// `DictationController.maxRecordingSeconds` : garde-fou, une dictée oubliée
/// s'arrête toute seule au bout de 10 min.
pub const MAX_RECORDING: Duration = Duration::from_secs(600);

/// Les touches qu'on sait distinguer. `Other` regroupe tout le reste : le
/// contenu exact n'a pas d'importance, seule compte la présence d'une autre
/// touche pendant un appui.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    AltLeft,
    AltRight,
    ControlLeft,
    ControlRight,
    ShiftLeft,
    ShiftRight,
    MetaLeft,
    MetaRight,
    /// F1… F24 (`Shortcut.fKeys` de macOS s'arrête à F15, rdev va plus loin).
    F(u8),
    Other,
}

impl Key {
    /// Une touche modificatrice ? Sert à `otherModifiersHeld` : une autre
    /// **touche** pendant l'appui n'est pas la même chose qu'un autre
    /// **modificateur** déjà tenu au moment d'appuyer, mais les deux rendent
    /// l'appui « sale ».
    pub fn is_modifier(self) -> bool {
        !matches!(self, Key::F(_) | Key::Other)
    }

    /// Nom court, pour l'affichage et la configuration (`plume-cli keys`).
    pub fn name(self) -> String {
        match self {
            Key::AltLeft => "alt_l".into(),
            Key::AltRight => "alt_r".into(),
            Key::ControlLeft => "ctrl_l".into(),
            Key::ControlRight => "ctrl_r".into(),
            Key::ShiftLeft => "shift_l".into(),
            Key::ShiftRight => "shift_r".into(),
            Key::MetaLeft => "super".into(),
            Key::MetaRight => "super_r".into(),
            Key::F(n) => format!("f{n}"),
            Key::Other => "autre".into(),
        }
    }

    /// Lit un nom de touche (`alt_l`, `alt_r`, `ctrl_r`, `super`, `f5`…).
    /// Insensible à la casse, `_` et `-` ignorés.
    pub fn parse(name: &str) -> Option<Key> {
        let n: String = name
            .chars()
            .filter(|c| *c != '_' && *c != '-' && *c != ' ')
            .flat_map(char::to_lowercase)
            .collect();
        match n.as_str() {
            "altl" | "alt" | "altleft" | "option" | "optionleft" => Some(Key::AltLeft),
            "altr" | "altright" | "altgr" | "optionright" => Some(Key::AltRight),
            "ctrll" | "controlleft" => Some(Key::ControlLeft),
            "ctrlr" | "controlright" => Some(Key::ControlRight),
            "shiftl" | "shiftleft" => Some(Key::ShiftLeft),
            "shiftr" | "shiftright" => Some(Key::ShiftRight),
            "super" | "metaleft" | "meta" | "cmd" | "win" => Some(Key::MetaLeft),
            "superr" | "metaright" => Some(Key::MetaRight),
            _ => {
                let f = n.strip_prefix('f')?;
                let num: u8 = f.parse().ok()?;
                (1..=24).contains(&num).then_some(Key::F(num))
            }
        }
    }
}

/// Un événement, horodaté (les durées sont la moitié du comportement : pas de
/// montre interne, c'est l'appelant qui donne l'heure).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    KeyDown { key: Key, at: Instant },
    KeyUp { key: Key, at: Instant },
    /// Une autre touche (n'importe laquelle, même une lettre), un clic ou un
    /// défilement. Pendant un appui, cela en fait un combo : l'appui est perdu.
    OtherKey,
}

/// Ce que la machine demande de faire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    Start,
    Stop { leet: bool },
    Cancel,
}

/// Le raccourci de dictée et son modificateur 1337.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GestureConfig {
    /// La touche de dictée. Choix proposés (comme sur macOS) : `AltLeft`,
    /// `AltRight`, `ControlRight`, `MetaRight`, ou une F-key.
    pub key: Key,
    /// Le modificateur tenu au moment du Stop pour sortir en 1337. `None` :
    /// pas de 1337.
    pub leet_key: Option<Key>,
    /// La fenêtre du double appui. macOS : 0,3 s (`doubleTapWindow`).
    pub double_tap_window: Duration,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            key: Key::AltLeft,
            leet_key: Some(Key::MetaLeft),
            double_tap_window: DOUBLE_TAP_WINDOW,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PendingStop {
    at: Instant,
    leet: bool,
}

/// La machine. `handle()` avale les événements, `flush()` fait expirer ce qui
/// doit expirer (la fenêtre du double appui, le garde-fou des 10 min).
#[derive(Clone, Debug)]
pub struct GestureMachine {
    config: GestureConfig,
    /// Les touches actuellement enfoncées (on ne voit que des appuis bruts, pas
    /// de drapeaux comme macOS).
    held: HashSet<Key>,
    /// Quand la touche de dictée est descendue, et si l'appui est resté propre.
    down_at: Option<Instant>,
    dirty: bool,
    /// Quand l'enregistrement a commencé (`None` : pas d'enregistrement).
    recording_since: Option<Instant>,
    /// Le Stop est armé, on attend la fenêtre du double appui.
    pending: Option<PendingStop>,
}

impl GestureMachine {
    pub fn new(config: GestureConfig) -> Self {
        Self {
            config,
            held: HashSet::new(),
            down_at: None,
            dirty: false,
            recording_since: None,
            pending: None,
        }
    }

    pub fn config(&self) -> &GestureConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: GestureConfig) {
        self.config = config;
    }

    pub fn is_recording(&self) -> bool {
        self.recording_since.is_some()
    }

    /// Un Stop attend la fin de la fenêtre du double appui.
    pub fn is_pending_stop(&self) -> bool {
        self.pending.is_some()
    }

    /// Absorbe un événement et rend ce qu'il faut faire.
    pub fn handle(&mut self, event: Event) -> Vec<Output> {
        let mut out = Vec::new();
        match event {
            Event::KeyDown { key, at } => {
                // Une répétition automatique (touche tenue) n'est pas un
                // nouvel appui : sans ça, un maintien se rallongerait sans fin
                // et ne serait jamais « long ».
                let first_press = self.held.insert(key);
                if key == self.config.key {
                    if first_press {
                        self.down_at = Some(at);
                        // Un autre modificateur déjà tenu (⇧ puis ⌥) : ce n'est
                        // pas un appui seul. Le modificateur 1337, lui, fait
                        // partie du geste.
                        self.dirty = self.other_modifier_held(key);
                    }
                } else if self.down_at.is_some() && Some(key) != self.config.leet_key {
                    // Une vraie touche pendant l'appui : ce n'était pas seul.
                    self.dirty = true;
                }
            }
            Event::KeyUp { key, at } => {
                self.held.remove(&key);
                if key == self.config.key {
                    if let Some(start) = self.down_at.take() {
                        let held = at.saturating_duration_since(start);
                        if !self.dirty && held < MAX_TAP_HOLD {
                            if let Some(output) = self.tap(at) {
                                out.push(output);
                            }
                        }
                    }
                }
                // macOS : un relâchement ne rend jamais l'appui « sale »
                // (`case .keyUp: break`).
            }
            Event::OtherKey => {
                if self.down_at.is_some() {
                    self.dirty = true;
                }
            }
        }
        out
    }

    /// Fait expirer le Stop armé et le garde-fou des 10 minutes. À appeler
    /// régulièrement (ou quand `next_deadline` est atteint).
    pub fn flush(&mut self, now: Instant) -> Option<Output> {
        if let Some(pending) = self.pending {
            if now.saturating_duration_since(pending.at) >= self.config.double_tap_window {
                self.pending = None;
                self.recording_since = None;
                return Some(Output::Stop { leet: pending.leet });
            }
        }
        if let Some(started) = self.recording_since {
            if now.saturating_duration_since(started) >= MAX_RECORDING {
                self.recording_since = None;
                self.pending = None;
                return Some(Output::Stop { leet: false });
            }
        }
        None
    }

    /// Le prochain instant où il faudra appeler `flush` (sinon `None`).
    pub fn next_deadline(&self, _now: Instant) -> Option<Instant> {
        let pending = self.pending.map(|p| p.at + self.config.double_tap_window);
        let watchdog = self.recording_since.map(|s| s + MAX_RECORDING);
        match (pending, watchdog) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Un appui valide vient d'avoir lieu (touche relâchée seule et vite).
    fn tap(&mut self, at: Instant) -> Option<Output> {
        if self.recording_since.is_none() {
            // Au départ, le geste 1337 compte comme un geste normal : le style
            // se choisit à la fin, on ne démarre jamais « en 1337 ».
            self.recording_since = Some(at);
            self.pending = None;
            Some(Output::Start)
        } else if self.pending.is_some() {
            // Deuxième appui rapide : annulé.
            self.pending = None;
            self.recording_since = None;
            Some(Output::Cancel)
        } else {
            // La fin décide du style, quel que soit le geste de départ.
            let leet = self.leet_held();
            self.pending = Some(PendingStop { at, leet });
            None
        }
    }

    fn leet_held(&self) -> bool {
        self.config
            .leet_key
            .is_some_and(|key| self.held.contains(&key))
    }

    /// Un modificateur autre que la touche de dictée (ou son côté opposé) est-il
    /// déjà tenu ?
    fn other_modifier_held(&self, key: Key) -> bool {
        self.held.iter().any(|held| {
            *held != key
                && Some(*held) != self.config.leet_key
                && held.is_modifier()
                && !same_modifier_other_side(*held, key)
        })
    }
}

/// ⌥ droite tenue pendant un appui sur ⌥ gauche : c'est le même modificateur,
/// ce n'est pas un autre.
fn same_modifier_other_side(a: Key, b: Key) -> bool {
    use Key::*;
    matches!(
        (a, b),
        (AltLeft, AltRight)
            | (AltRight, AltLeft)
            | (ControlLeft, ControlRight)
            | (ControlRight, ControlLeft)
            | (ShiftLeft, ShiftRight)
            | (ShiftRight, ShiftLeft)
            | (MetaLeft, MetaRight)
            | (MetaRight, MetaLeft)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Un petit banc d'essai : `at` est l'origine de la timeline.
    struct Bench {
        machine: GestureMachine,
        t0: Instant,
    }

    impl Bench {
        fn new() -> Self {
            Self::with(GestureConfig::default())
        }

        fn with(config: GestureConfig) -> Self {
            Self {
                machine: GestureMachine::new(config),
                t0: Instant::now(),
            }
        }

        fn down(&mut self, at: u64) -> Vec<Output> {
            self.machine.handle(Event::KeyDown {
                key: Key::AltLeft,
                at: self.t0 + ms(at),
            })
        }

        fn up(&mut self, at: u64) -> Vec<Output> {
            self.machine.handle(Event::KeyUp {
                key: Key::AltLeft,
                at: self.t0 + ms(at),
            })
        }

        /// Un appui complet (descente + relâchement) de `len` ms.
        fn tap(&mut self, at: u64, len: u64) -> Vec<Output> {
            let mut out = self.down(at);
            out.extend(self.up(at + len));
            out
        }

        fn other_down(&mut self, key: Key, at: u64) -> Vec<Output> {
            self.machine.handle(Event::KeyDown {
                key,
                at: self.t0 + ms(at),
            })
        }

        fn other_up(&mut self, key: Key, at: u64) -> Vec<Output> {
            self.machine.handle(Event::KeyUp {
                key,
                at: self.t0 + ms(at),
            })
        }

        fn other_key(&mut self) -> Vec<Output> {
            self.machine.handle(Event::OtherKey)
        }

        fn flush(&mut self, at: u64) -> Option<Output> {
            self.machine.flush(self.t0 + ms(at))
        }
    }

    #[test]
    fn tap_starts_the_dictation() {
        let mut b = Bench::new();
        assert_eq!(b.tap(0, 100), vec![Output::Start]);
        assert!(b.machine.is_recording());
    }

    #[test]
    fn tap_while_recording_stops_after_the_window() {
        let mut b = Bench::new();
        assert_eq!(b.tap(0, 100), vec![Output::Start]);
        // Appui de fin : rien tout de suite, le Stop est armé.
        assert_eq!(b.tap(2_000, 80), vec![]);
        assert!(b.machine.is_pending_stop());
        assert_eq!(b.flush(2_080 + 100), None);
        assert_eq!(
            b.flush(2_080 + 300),
            Some(Output::Stop { leet: false })
        );
        assert!(!b.machine.is_recording());
    }

    #[test]
    fn second_tap_within_the_window_cancels() {
        let mut b = Bench::new();
        assert_eq!(b.tap(0, 100), vec![Output::Start]);
        assert_eq!(b.tap(2_000, 80), vec![]);
        // Deuxième appui rapide : annulé, jamais de Stop.
        assert_eq!(b.tap(2_200, 80), vec![Output::Cancel]);
        assert!(!b.machine.is_recording());
        assert!(!b.machine.is_pending_stop());
        assert_eq!(b.flush(3_000), None);
    }

    #[test]
    fn second_tap_after_the_window_changes_nothing() {
        let mut b = Bench::new();
        assert_eq!(b.tap(0, 100), vec![Output::Start]);
        assert_eq!(b.tap(1_000, 50), vec![]);
        assert_eq!(b.flush(1_350), Some(Output::Stop { leet: false }));
        // Le Stop est parti : un nouvel appui démarre une autre dictée.
        assert_eq!(b.tap(2_000, 50), vec![Output::Start]);
    }

    #[test]
    fn long_press_is_ignored() {
        let mut b = Bench::new();
        assert_eq!(b.down(0), vec![]);
        // 0,9 s : au-delà de maxTapDuration.
        assert_eq!(b.up(900), vec![]);
        assert!(!b.machine.is_recording());
    }

    #[test]
    fn a_press_just_under_the_limit_is_still_a_tap() {
        let mut b = Bench::new();
        assert_eq!(b.tap(0, 799), vec![Output::Start]);
    }

    #[test]
    fn a_key_combined_with_the_dictation_key_is_ignored() {
        let mut b = Bench::new();
        assert_eq!(b.down(0), vec![]);
        // ⌥+E : la touche E descend pendant l'appui.
        assert_eq!(b.other_down(Key::Other, 50), vec![]);
        assert_eq!(b.up(100), vec![]);
        assert!(!b.machine.is_recording());
    }

    #[test]
    fn a_click_during_the_press_is_ignored() {
        let mut b = Bench::new();
        assert_eq!(b.down(0), vec![]);
        assert_eq!(b.other_key(), vec![]);
        assert_eq!(b.up(100), vec![]);
        assert!(!b.machine.is_recording());
    }

    #[test]
    fn another_modifier_already_held_makes_the_press_dirty() {
        let mut b = Bench::new();
        // ⇧ tenue, puis ⌥ : ⌥ ne sert pas à la dictée ici, il tape un caractère.
        assert_eq!(b.other_down(Key::ShiftLeft, 0), vec![]);
        assert_eq!(b.tap(100, 100), vec![]);
    }

    #[test]
    fn the_opposite_side_of_the_same_modifier_is_not_another_modifier() {
        // ⌥ droite tenue pendant un appui sur ⌥ gauche : macOS le tolère, un
        // appui reste un appui.
        let mut b = Bench::new();
        assert_eq!(b.other_down(Key::AltRight, 0), vec![]);
        assert_eq!(b.tap(100, 100), vec![Output::Start]);
    }

    #[test]
    fn releasing_another_key_does_not_spoil_the_press() {
        // macOS : `case .keyUp: break` — un relâchement n'est jamais « sale ».
        let mut b = Bench::new();
        assert_eq!(b.down(0), vec![]);
        assert_eq!(b.other_up(Key::Other, 50), vec![]);
        assert_eq!(b.up(100), vec![Output::Start]);
    }

    #[test]
    fn holding_the_leet_modifier_at_stop_gives_a_leet_stop() {
        let mut b = Bench::new();
        assert_eq!(b.tap(0, 100), vec![Output::Start]);
        // ⌘ (Super) tenu, puis appui de fin sur ⌥ : le 1337 se choisit à la fin.
        assert_eq!(b.other_down(Key::MetaLeft, 1_000), vec![]);
        assert_eq!(b.tap(1_200, 80), vec![]);
        assert_eq!(
            b.flush(1_280 + 300),
            Some(Output::Stop { leet: true })
        );
    }

    #[test]
    fn leet_held_at_start_still_starts_normally() {
        // On ne démarre jamais « en 1337 » : le style se choisit à la fin.
        let mut b = Bench::new();
        assert_eq!(b.other_down(Key::MetaLeft, 0), vec![]);
        assert_eq!(b.tap(100, 100), vec![Output::Start]);
        assert!(b.machine.is_recording());
    }

    #[test]
    fn leet_held_at_start_does_not_spoil_the_press() {
        // Le modificateur 1337 fait partie du geste : il n'est pas « une autre
        // touche ».
        let mut b = Bench::new();
        assert_eq!(b.other_down(Key::MetaLeft, 0), vec![]);
        assert_eq!(b.tap(100, 100), vec![Output::Start]);
        assert_eq!(b.tap(2_000, 80), vec![]);
        assert_eq!(
            b.flush(2_380),
            Some(Output::Stop { leet: true })
        );
    }

    #[test]
    fn leet_can_be_disabled() {
        let mut b = Bench::with(GestureConfig {
            leet_key: None,
            ..GestureConfig::default()
        });
        assert_eq!(b.tap(0, 100), vec![Output::Start]);
        assert_eq!(b.other_down(Key::MetaLeft, 1_000), vec![]);
        // ⌘ tenue : c'est une autre touche, l'appui n'est plus seul.
        assert_eq!(b.tap(1_200, 80), vec![]);
    }

    #[test]
    fn two_quick_taps_while_idle_start_then_cancel() {
        // Le geste réel : ⌥ (début), puis ⌥ ⌥ (annuler). Le premier des deux
        // arme le Stop, le second annule.
        let mut b = Bench::new();
        assert_eq!(b.tap(0, 100), vec![Output::Start]);
        assert_eq!(b.tap(2_000, 80), vec![]);
        assert_eq!(b.tap(2_250, 80), vec![Output::Cancel]);
        assert_eq!(b.flush(3_000), None);
    }

    #[test]
    fn an_f_key_can_be_the_dictation_key() {
        let mut b = Bench::with(GestureConfig {
            key: Key::F(5),
            ..GestureConfig::default()
        });
        b.machine.handle(Event::KeyDown {
            key: Key::F(5),
            at: b.t0,
        });
        let out = b.machine.handle(Event::KeyUp {
            key: Key::F(5),
            at: b.t0 + ms(100),
        });
        assert_eq!(out, vec![Output::Start]);
    }

    #[test]
    fn an_auto_repeat_does_not_extend_the_press() {
        let mut b = Bench::new();
        assert_eq!(b.down(0), vec![]);
        // La touche tenue renvoie des appuis : ils ne comptent pas comme un
        // nouvel appui (sinon le maintien ne serait jamais « long »).
        assert_eq!(b.down(400), vec![]);
        assert_eq!(b.up(900), vec![]);
        assert!(!b.machine.is_recording());
    }

    #[test]
    fn an_auto_repeat_does_not_start_a_second_dictation() {
        let mut b = Bench::new();
        assert_eq!(b.tap(0, 100), vec![Output::Start]);
        assert_eq!(b.down(1_000), vec![]);
        assert_eq!(b.down(1_400), vec![]);
        assert_eq!(b.up(1_500), vec![]);
        assert!(b.machine.is_recording());
        assert!(b.machine.is_pending_stop());
    }

    #[test]
    fn a_release_without_a_press_does_nothing() {
        let mut b = Bench::new();
        assert_eq!(b.up(100), vec![]);
        assert!(!b.machine.is_recording());
    }

    #[test]
    fn what_the_double_tap_window_contains() {
        // La fenêtre se compte depuis l'appui qui arme le Stop.
        let mut b = Bench::new();
        assert_eq!(b.tap(0, 100), vec![Output::Start]);
        assert_eq!(b.tap(1_000, 0), vec![]);
        assert_eq!(b.flush(1_299), None);
        assert_eq!(b.flush(1_300), Some(Output::Stop { leet: false }));
    }

    #[test]
    fn a_forgotten_dictation_stops_itself_after_ten_minutes() {
        let mut b = Bench::new();
        assert_eq!(b.tap(0, 100), vec![Output::Start]);
        assert_eq!(b.flush(600_099), None);
        assert_eq!(
            b.flush(600_100),
            Some(Output::Stop { leet: false })
        );
        assert!(!b.machine.is_recording());
    }

    #[test]
    fn the_next_deadline_is_the_armed_stop() {
        let mut b = Bench::new();
        assert_eq!(b.machine.next_deadline(b.t0), None);
        assert_eq!(b.tap(0, 100), vec![Output::Start]);
        // Pendant l'enregistrement : le garde-fou des 10 min.
        assert_eq!(
            b.machine.next_deadline(b.t0),
            Some(b.t0 + ms(100) + MAX_RECORDING)
        );
        assert_eq!(b.tap(1_000, 50), vec![]);
        assert_eq!(
            b.machine.next_deadline(b.t0),
            Some(b.t0 + ms(1_050) + DOUBLE_TAP_WINDOW)
        );
    }

    #[test]
    fn the_key_names_round_trip() {
        for key in [
            Key::AltLeft,
            Key::AltRight,
            Key::ControlRight,
            Key::MetaRight,
            Key::MetaLeft,
            Key::F(5),
        ] {
            assert_eq!(Key::parse(&key.name()), Some(key), "{}", key.name());
        }
        assert_eq!(Key::parse("F12"), Some(Key::F(12)));
        assert_eq!(Key::parse("nawak"), None);
    }
}
