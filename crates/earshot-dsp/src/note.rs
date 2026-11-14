//! Converting between frequencies, MIDI note numbers and note names.

const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Fractional MIDI note number for a frequency. A4 (440 Hz) is 69, low E is 40.
pub fn hz_to_midi(hz: f32) -> f32 {
    69.0 + 12.0 * (hz / 440.0).log2()
}

pub fn midi_to_hz(midi: f32) -> f32 {
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

/// Name of the nearest note with its octave, e.g. 40 is "E2".
pub fn note_name(midi: f32) -> String {
    let n = midi.round() as i32;
    format!(
        "{}{}",
        NAMES[n.rem_euclid(12) as usize],
        n.div_euclid(12) - 1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concert_a() {
        assert_eq!(hz_to_midi(440.0), 69.0);
        assert!((midi_to_hz(69.0) - 440.0).abs() < 1e-3);
    }

    #[test]
    fn names() {
        assert_eq!(note_name(40.0), "E2");
        assert_eq!(note_name(64.2), "E4");
        assert_eq!(note_name(61.0), "C#4");
        assert_eq!(note_name(hz_to_midi(110.0)), "A2");
    }
}
