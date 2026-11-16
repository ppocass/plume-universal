//! Le côté entrée de Plume : le geste de dictée, l'écoute clavier globale, le
//! micro et le collage. Le pendant Linux/Windows de `HotkeyMonitor`,
//! `Shortcut`, `DictationController` (les gestes), `AudioRecorder` et
//! `TextInserter` de l'app macOS.
//!
//! Tout est sans interface : `plume-cli` s'en sert pour tester à la main, l'app
//! Tauri s'en servira pour de vrai.

pub mod gesture;
pub mod listener;
pub mod mic;
pub mod paste;

pub use gesture::{Event, GestureConfig, GestureMachine, Key, Output};
pub use listener::start as listen;
pub use mic::{Mic, SAMPLE_RATE};
pub use paste::paste;
