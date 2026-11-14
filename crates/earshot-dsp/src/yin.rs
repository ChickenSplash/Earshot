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

        let first = (min_lag..=max_lag).find(|&lag| d[lag] < threshold)?;
        let mut lag = bottom_of_dip(d, first, max_lag);

        // Octave check. A note also nearly repeats at half its period when its
        // fundamental is weak (common for low E through a mic), so the first dip
        // can be an overtone. A true note repeats about as well at 2 and 3 periods,
        // so only move to one of those if it is clearly the better match.
        let first_lag = lag;
        for multiple in [2, 3] {
            let centre = first_lag * multiple;
            if centre + multiple > max_lag {
                break;
            }
            let candidate = (centre - multiple..=centre + multiple)
                .min_by(|&a, &b| d[a].total_cmp(&d[b]))
                .unwrap();
            if d[candidate] < d[lag] * OCTAVE_DEEPER && d[lag] - d[candidate] > OCTAVE_MARGIN {
                lag = candidate;
            }
        }

        // Fit a parabola through the dip for a pitch between whole samples.
        let (a, b, c) = (d[lag - 1], d[lag], d[lag + 1]);
        let curve = a - 2.0 * b + c;
        let shift = if curve != 0.0 {
            (a - c) / (2.0 * curve)
        } else {
            0.0
        };
        Some(Pitch {
            hz: self.sample_rate / (lag as f32 + shift),
            clarity: (1.0 - b).clamp(0.0, 1.0),
        })
    }
}

/// A longer period must be at least this much deeper (smaller) to replace the first dip.
const OCTAVE_DEEPER: f32 = 0.5;
/// ...and deeper by at least this much, so two near-perfect dips on a clean note don't count.
const OCTAVE_MARGIN: f32 = 0.03;

/// Walk down from `lag` to the bottom of the dip it is in.
fn bottom_of_dip(d: &[f32], mut lag: usize, max_lag: usize) -> usize {
    while lag < max_lag && d[lag + 1] < d[lag] {
        lag += 1;
    }
    lag
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_signal::{RATE, harmonics, string, with_noise};
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

    /// Through a small mic, a low string's fundamental can be quieter than its
    /// overtones, so the signal nearly repeats every half period.
    const WEAK_FUNDAMENTAL: [f32; 6] = [0.1, 1.0, 0.1, 0.5, 0.05, 0.3];

    fn assert_reads(yin: &mut Yin, samples: &[f32], hz: f32) {
        let pitch = yin
            .detect(samples)
            .unwrap_or_else(|| panic!("no pitch found for {hz} Hz"));
        assert!(
            cents_off(pitch.hz, hz).abs() < 5.0,
            "{hz} Hz read as {} Hz",
            pitch.hz
        );
    }

    #[test]
    fn low_e_with_a_weak_fundamental_is_not_read_an_octave_up() {
        let hz = midi_to_hz(40.0);
        assert_reads(
            &mut Yin::new(RATE, 60.0, 1000.0),
            &harmonics(hz, &WEAK_FUNDAMENTAL, 4096),
            hz,
        );
    }

    #[test]
    fn weak_fundamentals_on_the_low_strings_with_noise() {
        let mut yin = Yin::new(RATE, 60.0, 1000.0);
        for midi in [40, 41, 43, 45, 47, 50] {
            let hz = midi_to_hz(midi as f32);
            assert_reads(
                &mut yin,
                &with_noise(harmonics(hz, &WEAK_FUNDAMENTAL, 4096), 0.02),
                hz,
            );
        }
    }

    #[test]
    fn real_higher_notes_are_not_pulled_down_an_octave() {
        let mut yin = Yin::new(RATE, 60.0, 1000.0);
        // E3, A3, D4, G4, B4, E5: a true note matches as well at 2 and 3 periods.
        for midi in [52, 57, 62, 67, 71, 76] {
            let hz = midi_to_hz(midi as f32);
            assert_reads(&mut yin, &string(hz, 4096), hz);
            assert_reads(&mut yin, &with_noise(string(hz, 4096), 0.05), hz);
        }
    }

    #[test]
    fn silence_and_noise_have_no_pitch() {
        let mut yin = Yin::new(RATE, 60.0, 1000.0);
        assert_eq!(yin.detect(&[0.0; 4096]), None);
        assert_eq!(yin.detect(&with_noise(vec![0.0; 4096], 0.5)), None);
    }

    #[test]
    fn too_short_a_window() {
        let mut yin = Yin::new(RATE, 60.0, 1000.0);
        assert_eq!(yin.detect(&string(110.0, 512)), None);
    }
}
