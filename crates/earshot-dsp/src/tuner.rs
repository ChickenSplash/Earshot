//! Turns a stream of pitch readings into a steady tuner display.

use crate::hz_to_midi;

/// E2 A2 D3 G3 B3 E4 as MIDI notes, low string first.
pub const STANDARD_TUNING: [u8; 6] = [40, 45, 50, 55, 59, 64];

const IN_TUNE_CENTS: f32 = 5.0;
/// Weight kept from the previous reading each update. Higher is steadier but slower.
const SMOOTHING: f32 = 0.7;
/// Keep showing the last reading this long after the note fades.
const HOLD_SECS: f64 = 1.5;
/// Readings in a row needed to switch to a different note, so a single
/// octave slip or pick-attack glitch can't flip the display.
const CONFIRM_READINGS: u32 = 3;

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
    /// A different note being heard, and how many readings in a row agreed on it.
    candidate: Option<(f32, u32)>,
    last_heard: f64,
}

impl Tuner {
    pub fn new(strings: &[u8]) -> Self {
        assert!(!strings.is_empty());
        Self {
            strings: strings.to_vec(),
            smoothed: None,
            candidate: None,
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
                match self.smoothed {
                    // Smooth small wobbles.
                    Some(prev) if (midi - prev).abs() < 1.0 => {
                        self.smoothed = Some(prev * SMOOTHING + midi * (1.0 - SMOOTHING));
                        self.candidate = None;
                    }
                    // Jump to a different note once it has been heard a few times running.
                    _ => {
                        let count = match self.candidate {
                            Some((seen, count)) if (midi - seen).abs() < 1.0 => count + 1,
                            _ => 1,
                        };
                        if count >= CONFIRM_READINGS {
                            self.smoothed = Some(midi);
                            self.candidate = None;
                        } else {
                            self.candidate = Some((midi, count));
                        }
                    }
                }
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

    /// Feeds the same note enough times to be confirmed, returning the last reading.
    fn hear(tuner: &mut Tuner, midi: f32, now: f64) -> Option<Reading> {
        (0..CONFIRM_READINGS)
            .map(|_| tuner.update(Some(midi_to_hz(midi)), now))
            .last()
            .unwrap()
    }

    #[test]
    fn picks_the_nearest_string() {
        let mut tuner = Tuner::new(&STANDARD_TUNING);
        let r = hear(&mut tuner, 54.7, 0.0).unwrap();
        assert_eq!(r.string, 3); // G3
        assert!((r.cents + 30.0).abs() < 0.1);
        assert!(!r.in_tune);
    }

    #[test]
    fn in_tune_within_five_cents() {
        let mut tuner = Tuner::new(&STANDARD_TUNING);
        assert!(hear(&mut tuner, 40.03, 0.0).unwrap().in_tune);
    }

    #[test]
    fn holds_then_clears() {
        let mut tuner = Tuner::new(&STANDARD_TUNING);
        hear(&mut tuner, 45.0, 0.0);
        assert!(tuner.update(None, 1.0).is_some());
        assert!(tuner.update(None, 2.0).is_none());
    }

    #[test]
    fn a_single_reading_is_not_enough_to_show_a_note() {
        let mut tuner = Tuner::new(&STANDARD_TUNING);
        assert!(tuner.update(Some(midi_to_hz(45.0)), 0.0).is_none());
    }

    #[test]
    fn jumps_to_a_new_string_once_confirmed() {
        let mut tuner = Tuner::new(&STANDARD_TUNING);
        hear(&mut tuner, 40.0, 0.0);
        let r = hear(&mut tuner, 64.0, 0.1).unwrap();
        assert_eq!(r.string, 5);
        assert!(r.cents.abs() < 0.1);
    }

    #[test]
    fn ignores_a_one_off_octave_slip() {
        let mut tuner = Tuner::new(&STANDARD_TUNING);
        hear(&mut tuner, 40.0, 0.0);
        let slip = tuner.update(Some(midi_to_hz(52.0)), 0.1).unwrap(); // E3
        assert_eq!(slip.string, 0);
        assert!(slip.cents.abs() < 0.1);
        let back = tuner.update(Some(midi_to_hz(40.0)), 0.2).unwrap();
        assert_eq!(back.string, 0);
    }
}
