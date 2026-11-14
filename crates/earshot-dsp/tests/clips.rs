//! Note detection against real recordings of an acoustic guitar through a USB mic.
//!
//! The expected pluck times were read off the waveforms by eye, to about 5 ms.
//! The fast re-plucks on a ringing low E are only clear to about 15 ms.

#[path = "../support/wav.rs"]
mod wav;

use earshot_dsp::{NoteDetector, NoteEvent, note_name};

struct Clip {
    file: &'static str,
    /// (seconds, MIDI note) of every pluck that must be found.
    plucks: &'static [(f64, u8)],
    /// Times of sounds that may or may not count as a pluck (pick ticks on a
    /// ringing string). A note there is allowed if it has the expected pitch.
    maybe: &'static [(f64, u8)],
    tolerance: f64,
}

const OPEN: f64 = 0.015;

const CLIPS: &[Clip] = &[
    Clip {
        file: "open-e2",
        plucks: &[(5.06, 40), (6.58, 40), (8.12, 40)],
        maybe: &[],
        tolerance: OPEN,
    },
    Clip {
        file: "open-a2",
        plucks: &[(5.04, 45), (6.45, 45), (7.85, 45)],
        maybe: &[],
        tolerance: OPEN,
    },
    Clip {
        file: "open-d3",
        plucks: &[(5.39, 50), (6.68, 50), (8.16, 50)],
        maybe: &[],
        tolerance: OPEN,
    },
    Clip {
        file: "open-g3",
        plucks: &[(4.39, 55), (5.78, 55), (7.32, 55)],
        maybe: &[],
        tolerance: OPEN,
    },
    Clip {
        file: "open-b3",
        plucks: &[(4.74, 59), (6.12, 59), (7.50, 59)],
        maybe: &[],
        tolerance: OPEN,
    },
    Clip {
        file: "open-e4",
        plucks: &[(3.02, 64), (4.54, 64), (6.12, 64)],
        maybe: &[],
        tolerance: OPEN,
    },
    // A2 up to A3 one fret at a time. Between notes there are finger releases,
    // a squeak at 10.33 s and fretting taps, none of which are notes.
    Clip {
        file: "chromatic-a",
        plucks: &[
            (2.76, 45),
            (4.56, 46),
            (6.10, 47),
            (7.62, 48),
            (9.33, 49),
            (10.92, 50),
            (12.42, 51),
            (13.93, 52),
            (15.39, 53),
            (16.96, 54),
            (18.43, 55),
            (19.99, 56),
            (21.65, 57),
        ],
        maybe: &[(19.30, 55)],
        tolerance: OPEN,
    },
    // About 14 hard plucks on low E, many while it is still ringing.
    Clip {
        file: "low-e-hard",
        plucks: &[
            (3.29, 40),
            (3.90, 40),
            (4.22, 40),
            (4.56, 40),
            (5.23, 40),
            (7.18, 40),
            (8.17, 40),
            (8.47, 40),
            (8.80, 40),
            (10.33, 40),
            (10.63, 40),
            (10.97, 40),
        ],
        maybe: &[(3.57, 40), (4.89, 40), (7.53, 40), (7.84, 40), (9.11, 40)],
        tolerance: 0.025,
    },
    Clip {
        file: "quiet",
        plucks: &[],
        maybe: &[],
        tolerance: OPEN,
    },
];

fn detect(samples: &[f32], rate: f32, block: usize) -> Vec<NoteEvent> {
    let mut detector = NoteDetector::new(rate);
    let mut notes = Vec::new();
    for chunk in samples.chunks(block) {
        detector.push(chunk, &mut notes);
    }
    detector.flush(&mut notes);
    notes
}

fn clip_path(name: &str) -> String {
    format!("{}/../../clips/{name}.wav", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn finds_every_pluck_and_nothing_else() {
    let mut problems = Vec::new();
    for clip in CLIPS {
        let (samples, rate) = wav::load(&clip_path(clip.file));
        let notes = detect(&samples, rate, 256);
        let near = |&(t, midi): &(f64, u8), n: &NoteEvent| {
            (n.time - t).abs() <= clip.tolerance && n.midi.round() as u8 == midi
        };

        for pluck in clip.plucks {
            if !notes.iter().any(|n| near(pluck, n)) {
                problems.push(format!(
                    "{}: missed {} at {:.2}s",
                    clip.file,
                    note_name(pluck.1 as f32),
                    pluck.0
                ));
            }
        }
        for n in &notes {
            let expected = clip.plucks.iter().chain(clip.maybe).any(|p| near(p, n));
            if !expected {
                problems.push(format!(
                    "{}: unexpected {} at {:.3}s",
                    clip.file,
                    note_name(n.midi),
                    n.time
                ));
            }
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn block_size_does_not_change_the_result() {
    // Live audio arrives in blocks of whatever size the system picks.
    let (samples, rate) = wav::load(&clip_path("chromatic-a"));
    let reference = detect(&samples, rate, 256);
    for block in [1, 64, 480, 4096] {
        assert_eq!(
            detect(&samples, rate, block),
            reference,
            "block size {block}"
        );
    }
}
