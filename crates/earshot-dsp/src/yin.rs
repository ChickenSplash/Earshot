//! YIN pitch detection: finds the exact pitch of a single note.
//!
//! The idea: a note repeats itself every period. YIN compares the signal with
//! a delayed copy of itself and finds the smallest delay (lag) at which they
//! nearly match. That lag is one period, so the pitch is `sample_rate / lag`.
//! It only works for one note at a time; chords need a different approach.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pitch {
    pub hz: f32,
    /// How cleanly the signal repeats, 0 to 1. Above about 0.85 is a clear note.
    pub clarity: f32,
}

pub struct Yin {
    sample_rate: f32,
    min_lag: usize,
    max_lag: usize,
    /// A dip must fall below this to count as a period (lower is stricter).
    threshold: f32,
    diff: Vec<f32>,
}

impl Yin {
    /// `min_hz` sets how much audio is needed: the window must hold at least
    /// two periods of the lowest note, plus some margin for a clean reading.
    pub fn new(sample_rate: f32, min_hz: f32, max_hz: f32) -> Self {
        let max_lag = (sample_rate / min_hz).ceil() as usize;
        Self {
            sample_rate,
            min_lag: (sample_rate / max_hz).floor().max(2.0) as usize,
            max_lag,
            threshold: 0.15,
            diff: vec![1.0; max_lag + 2],
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// The pitch of `samples`, or `None` if nothing repeats clearly enough.
    pub fn detect(&mut self, samples: &[f32]) -> Option<Pitch> {
        let Self {
            min_lag,
            max_lag,
            threshold,
            ..
        } = *self;
        if samples.len() < 2 * max_lag + 2 {
            return None;
        }
        let size = samples.len() - max_lag - 1;
        let d = &mut self.diff;

        // Squared difference at each lag, normalised by its running mean so
        // 0 is a perfect repeat and values near 1 mean no repeat at all.
        d[0] = 1.0;
        let mut running = 0.0;
        for lag in 1..=max_lag + 1 {
            let sum: f32 = samples[..size]
                .iter()
                .zip(&samples[lag..lag + size])
                .map(|(a, b)| (a - b) * (a - b))
                .sum();
            running += sum;
            d[lag] = if running > 0.0 {
                sum * lag as f32 / running
            } else {
                1.0
            };
        }

        let mut lag = min_lag;
        while lag <= max_lag {
            if d[lag] < threshold {
                // Walk down to the bottom of the dip.
                while lag < max_lag && d[lag + 1] < d[lag] {
                    lag += 1;
                }
                // Fit a parabola through the dip for a pitch between whole samples.
                let (a, b, c) = (d[lag - 1], d[lag], d[lag + 1]);
                let curve = a - 2.0 * b + c;
                let shift = if curve != 0.0 {
                    (a - c) / (2.0 * curve)
                } else {
                    0.0
                };
                return Some(Pitch {
                    hz: self.sample_rate / (lag as f32 + shift),
                    clarity: (1.0 - b).clamp(0.0, 1.0),
                });
            }
            lag += 1;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_signal::{RATE, string};
    use crate::{STANDARD_TUNING, hz_to_midi, midi_to_hz};

    fn cents_off(found: f32, expected: f32) -> f32 {
        (hz_to_midi(found) - hz_to_midi(expected)) * 100.0
    }

    #[test]
    fn finds_every_open_string() {
        let mut yin = Yin::new(RATE, 60.0, 1000.0);
        for midi in STANDARD_TUNING {
            let hz = midi_to_hz(midi as f32);
            let pitch = yin.detect(&string(hz, 4096)).expect("no pitch found");
            assert!(
                cents_off(pitch.hz, hz).abs() < 1.0,
                "{hz} Hz read as {}",
                pitch.hz
            );
            assert!(pitch.clarity > 0.9);
        }
    }

    #[test]
    fn hears_a_slightly_flat_string() {
        let mut yin = Yin::new(RATE, 60.0, 1000.0);
        let hz = midi_to_hz(45.0 - 0.2); // A2, 20 cents flat
        let pitch = yin.detect(&string(hz, 4096)).unwrap();
        assert!((cents_off(pitch.hz, midi_to_hz(45.0)) + 20.0).abs() < 1.0);
    }

    #[test]
    fn silence_and_noise_have_no_pitch() {
        let mut yin = Yin::new(RATE, 60.0, 1000.0);
        assert_eq!(yin.detect(&[0.0; 4096]), None);

        // Cheap deterministic white noise.
        let mut seed = 1u32;
        let noise: Vec<f32> = (0..4096)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 8) as f32 / (1 << 24) as f32 - 0.5
            })
            .collect();
        assert_eq!(yin.detect(&noise), None);
    }

    #[test]
    fn too_short_a_window() {
        let mut yin = Yin::new(RATE, 60.0, 1000.0);
        assert_eq!(yin.detect(&string(110.0, 512)), None);
    }
}
