//! L'écouteur clavier global (X11 et Windows) — le pendant de
//! `HotkeyMonitor.swift`.
//!
//! **Écoute seule** : `rdev` lit les événements par XRecord sous X11 (aucun
//! droit root, aucun « grab ») et par un hook bas niveau sous Windows. Rien
//! n'est bloqué ni modifié : les raccourcis des autres apps continuent de
//! marcher, comme le tap **actif** de macOS qui renvoie chaque événement tel
//! quel.
//!
//! Tout ce qui est décidé ici est mécanique : traduire les événements `rdev`
//! en [`Event`], et laisser la [machine à états](super::gesture) trancher. Le
//! canal rendu ne porte que `Start` / `Stop` / `Cancel`.

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use super::gesture::{Event, GestureConfig, GestureMachine, Key, Output};

/// Combien de temps on laisse à `rdev` pour dire qu'il n'a pas pu démarrer
/// (pas de `DISPLAY`, extension RECORD absente…). Au-delà, on considère que
/// l'écoute tourne : `rdev::listen` ne rend la main que s'il échoue.
const START_TIMEOUT: Duration = Duration::from_millis(400);

/// Démarre l'écoute globale et rend le canal des gestes.
///
/// Le canal se ferme quand l'écoute s'arrête : `for output in rx` termine tout
/// seul.
pub fn start(config: GestureConfig) -> Result<Receiver<Output>> {
    let (raw_tx, raw_rx) = channel::<Event>();
    let (out_tx, out_rx) = channel::<Output>();
    let (status_tx, status_rx) = channel::<Result<(), rdev::ListenError>>();

    // Le pilote : il tient la machine, donc les horodatages et les échéances.
    thread::Builder::new()
        .name("plume-geste".into())
        .spawn(move || drive(raw_rx, out_tx, GestureMachine::new(config)))?;

    // L'écoute brute.
    thread::Builder::new()
        .name("plume-rdev".into())
        .spawn(move || {
            let result = rdev::listen(move |event| {
                if let Some(event) = gesture_event(event) {
                    // Le pilote s'est arrêté : plus rien à faire.
                    let _ = raw_tx.send(event);
                }
            });
            let _ = status_tx.send(result);
        })?;

    match status_rx.recv_timeout(START_TIMEOUT) {
        Ok(Err(error)) => Err(anyhow!("écoute clavier impossible : {error:?}")),
        _ => Ok(out_rx),
    }
}

/// Le pilote : un événement à la fois, puis on fait expirer ce qui doit
/// expirer (fenêtre du double appui, garde-fou des 10 min).
fn drive(raw: Receiver<Event>, out: Sender<Output>, mut machine: GestureMachine) {
    loop {
        let now = Instant::now();
        let timeout = machine
            .next_deadline(now)
            .map(|deadline| deadline.saturating_duration_since(now))
            .unwrap_or(Duration::from_secs(3_600));
        match raw.recv_timeout(timeout) {
            Ok(event) => {
                for output in machine.handle(event) {
                    if out.send(output).is_err() {
                        return;
                    }
                }
                drain(&mut machine, &out);
            }
            Err(RecvTimeoutError::Timeout) => drain(&mut machine, &out),
            Err(RecvTimeoutError::Disconnected) => {
                // L'écoute s'est arrêtée : on ne laisse pas un Stop en suspens.
                drain(&mut machine, &out);
                return;
            }
        }
    }
}

fn drain(machine: &mut GestureMachine, out: &Sender<Output>) {
    let now = Instant::now();
    while let Some(output) = machine.flush(now) {
        if out.send(output).is_err() {
            return;
        }
    }
}

/// Traduit un événement `rdev` en événement de geste. `None` : il ne compte pas
/// (un simple déplacement de souris n'est pas « une autre touche » — macOS ne
/// salit pas un appui pour ça).
fn gesture_event(event: rdev::Event) -> Option<Event> {
    gesture_of(event.event_type)
}

fn gesture_of(kind: rdev::EventType) -> Option<Event> {
    let at = Instant::now();
    match kind {
        rdev::EventType::KeyPress(key) => Some(Event::KeyDown {
            key: key_of(key),
            at,
        }),
        rdev::EventType::KeyRelease(key) => Some(Event::KeyUp {
            key: key_of(key),
            at,
        }),
        rdev::EventType::ButtonPress(_)
        | rdev::EventType::ButtonRelease(_)
        | rdev::EventType::Wheel { .. } => Some(Event::OtherKey),
        rdev::EventType::MouseMove { .. } => None,
    }
}

/// Les touches qu'on suit. Tout le reste devient [`Key::Other`] : on n'a pas
/// besoin de savoir laquelle, seulement qu'une autre touche est intervenue.
fn key_of(key: rdev::Key) -> Key {
    match key {
        // Sous X11, Alt_L arrive en `Alt` et Alt_R en `AltGr`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_keep_their_side() {
        assert_eq!(key_of(rdev::Key::Alt), Key::AltLeft);
        assert_eq!(key_of(rdev::Key::AltGr), Key::AltRight);
        assert_eq!(key_of(rdev::Key::ControlRight), Key::ControlRight);
        assert_eq!(key_of(rdev::Key::MetaRight), Key::MetaRight);
        assert_eq!(key_of(rdev::Key::F5), Key::F(5));
        assert_eq!(key_of(rdev::Key::KeyA), Key::Other);
    }

    #[test]
    fn a_key_press_becomes_a_key_down() {
        let event = gesture_of(rdev::EventType::KeyPress(rdev::Key::Alt)).unwrap();
        match event {
            Event::KeyDown { key, .. } => assert_eq!(key, Key::AltLeft),
            other => panic!("attendu KeyDown, reçu {other:?}"),
        }
    }
}
