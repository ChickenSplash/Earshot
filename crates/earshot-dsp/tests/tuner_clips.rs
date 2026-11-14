//! The tuner against real recordings, fed the way the app feeds it: a pitch
//! reading every frame from the newest audio, plus the note detector's plucks.

#[path = "../support/wav.rs"]
mod wav;

use earshot_dsp::{NoteDetector, STANDARD_TUNING, Tuner, Yin, level_db, note_name};

const FPS: f32 = 60.0;
const WINDOW: usize = 4096;
const GATE_DB: f32 = -50.0;

/// Every string the tuner showed while the clip played, with the time.
fn strings_shown(file: &str) -> Vec<(f32, usize)> {
    let path = format!("{}/../../clips/{file}.wav", env!("CARGO_MANIFEST_DIR"));
    let (samples, rate) = wav::load(&path);
    let mut yin = Yin::new(rate, 60.0, 1000.0);
    let mut tuner = Tuner::new(&STANDARD_TUNING);
    let mut detector = NoteDetector::new(rate);
    let mut notes = Vec::new();
    let frame = (rate / FPS) as usize;

    let mut shown = Vec::new();
    let mut end = 0;
    while end + frame <= samples.len() {
        detector.push(&samples[end..end + frame], &mut notes);
        end += frame;
        for note in notes.drain(..) {
            tuner.plucked(note.midi);
        }
        let window = &samples[end.saturating_sub(WINDOW)..end];
        let hz = (level_db(window) > GATE_DB)
            .then(|| yin.detect(window).map(|p| p.hz))
            .flatten();
        let now = end as f32 / rate;
        if let Some(reading) = tuner.update(hz, now as f64) {
            shown.push((now, reading.string));
        }
    }
    shown
}

#[test]
fn shows_only_the_string_being_played() {
    let clips = [
        ("open-e2", 0),
        ("open-a2", 1),
        ("open-d3", 2),
        ("open-g3", 3),
        ("open-b3", 4),
        ("open-e4", 5),
        // Ends with a long fade where the low E's overtone outlasts it.
        ("low-e-hard", 0),
    ];
    let mut problems = Vec::new();
    for (file, string) in clips {
        let wrong: Vec<_> = strings_shown(file)
            .into_iter()
            .filter(|&(_, s)| s != string)
            .collect();
        if let (Some(first), Some(last)) = (wrong.first(), wrong.last()) {
            problems.push(format!(
                "{file}: showed {} from {:.2}s to {:.2}s",
                note_name(STANDARD_TUNING[first.1] as f32),
                first.0,
                last.0
            ));
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}
