//! Turns a stream of pitch readings into a steady tuner display.

use crate::hz_to_midi;

/// E2 A2 D3 G3 B3 E4 as MIDI notes, low string first.
pub const STANDARD_TUNING: [u8; 6] = [40, 45, 50, 55, 59, 64];

const IN_TUNE_CENTS: f32 = 5.0;
/// Weight kept from the previous reading each update. Higher is steadier but slower.
const SMOOTHING: f32 = 0.7;
/// Keep showing the last reading this long after the note fades.
const HOLD_SECS: f64 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    /// Smoothed fractional MIDI note of what is being played.
    pub midi: f32,
    /// Index into the tuning of the nearest string.
    pub string: usize,
    /// How far from that string, negative is flat, positive is sharp.
    pub cents: f32,
    pub in_tune: bool,
}

pub struct Tuner {
    strings: Vec<u8>,
    smoothed: Option<f32>,
    last_heard: f64,
}

impl Tuner {
    pub fn new(strings: &[u8]) -> Self {
        assert!(!strings.is_empty());
        Self {
            strings: strings.to_vec(),
            smoothed: None,
            last_heard: 0.0,
        }
    }

    pub fn strings(&self) -> &[u8] {
        &self.strings
    }

    /// Feed the latest pitch (or `None` when nothing clear is heard) at time `now` in seconds.
    pub fn update(&mut self, hz: Option<f32>, now: f64) -> Option<Reading> {
        match hz {
            Some(hz) => {
                let midi = hz_to_midi(hz);
                // Smooth small wobbles, but jump straight to a different note.
                self.smoothed = Some(match self.smoothed {
                    Some(prev) if (midi - prev).abs() < 1.0 => {
                        prev * SMOOTHING + midi * (1.0 - SMOOTHING)
                    }
                    _ => midi,
                });
                self.last_heard = now;
            }
            None if now - self.last_heard > HOLD_SECS => self.smoothed = None,
            None => {}
        }
        self.smoothed.map(|midi| self.reading(midi))
    }

    fn reading(&self, midi: f32) -> Reading {
        let (string, &target) = self
            .strings
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (midi - **a as f32)
                    .abs()
                    .total_cmp(&(midi - **b as f32).abs())
            })
            .unwrap();
        let cents = (midi - target as f32) * 100.0;
        Reading {
            midi,
            string,
            cents,
            in_tune: cents.abs() <= IN_TUNE_CENTS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi_to_hz;

    #[test]
    fn picks_the_nearest_string() {
        let mut tuner = Tuner::new(&STANDARD_TUNING);
        let r = tuner.update(Some(midi_to_hz(54.7)), 0.0).unwrap();
        assert_eq!(r.string, 3); // G3
        assert!((r.cents + 30.0).abs() < 0.1);
        assert!(!r.in_tune);
    }

    #[test]
    fn in_tune_within_five_cents() {
        let mut tuner = Tuner::new(&STANDARD_TUNING);
        assert!(tuner.update(Some(midi_to_hz(40.03)), 0.0).unwrap().in_tune);
    }

    #[test]
    fn holds_then_clears() {
        let mut tuner = Tuner::new(&STANDARD_TUNING);
        tuner.update(Some(110.0), 0.0);
        assert!(tuner.update(None, 1.0).is_some());
        assert!(tuner.update(None, 2.0).is_none());
    }

    #[test]
    fn jumps_to_a_new_string_without_smoothing() {
        let mut tuner = Tuner::new(&STANDARD_TUNING);
        tuner.update(Some(midi_to_hz(40.0)), 0.0);
        let r = tuner.update(Some(midi_to_hz(64.0)), 0.1).unwrap();
        assert_eq!(r.string, 5);
        assert!(r.cents.abs() < 0.1);
    }
}
