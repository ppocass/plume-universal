//! Module audio : la barrière anti-silence, le découpage et la lecture des WAV.
pub mod gate;
pub mod wav;

pub use gate::{analyse, chunks, GateVerdict};
pub use wav::read_wav_16k_mono;
