//! Signal processing for Earshot: turning raw microphone samples into notes.
//!
//! Kept free of Bevy and audio I/O so it builds fast and is easy to test.

pub mod note;
pub mod tuner;
pub mod yin;

pub use note::{hz_to_midi, midi_to_hz, note_name};
pub use tuner::{Reading, STANDARD_TUNING, Tuner};
pub use yin::{Pitch, Yin};

/// RMS level of a block of samples in dBFS (0 is full scale, silence is very negative).
pub fn level_db(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return f32::NEG_INFINITY;
    }
    let mean_square = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    10.0 * mean_square.max(1e-12).log10()
}

#[cfg(test)]
pub(crate) mod test_signal {
    use std::f32::consts::TAU;

    pub const RATE: f32 = 48_000.0;

    /// A plucked-string-like tone: a fundamental plus decaying harmonics.
    pub fn string(hz: f32, len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| {
                let t = i as f32 / RATE;
                (1..=6)
                    .map(|h| (TAU * hz * h as f32 * t).sin() * 0.5 / h as f32)
                    .sum()
            })
            .collect()
    }
}
