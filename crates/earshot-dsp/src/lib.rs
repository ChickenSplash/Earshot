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
        harmonics(hz, &[1.0, 0.5, 0.33, 0.25, 0.2, 0.17], len)
    }

    /// A tone with the given amplitude for each harmonic, fundamental first.
    pub fn harmonics(hz: f32, amps: &[f32], len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| {
                let t = i as f32 / RATE;
                amps.iter()
                    .enumerate()
                    .map(|(h, a)| a * 0.3 * (TAU * hz * (h + 1) as f32 * t).sin())
                    .sum()
            })
            .collect()
    }

    /// Adds deterministic white noise at `level` (peak amplitude).
    pub fn with_noise(mut samples: Vec<f32>, level: f32) -> Vec<f32> {
        let mut seed = 1u32;
        for s in &mut samples {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *s += ((seed >> 8) as f32 / (1 << 24) as f32 - 0.5) * 2.0 * level;
        }
        samples
    }
}
