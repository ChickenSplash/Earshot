//! Lists the notes the note detector finds in WAV files.
//!
//! cargo run -p earshot-dsp --example notes -- clips/*.wav

#[path = "../support/wav.rs"]
mod wav;

use earshot_dsp::{NoteDetector, note_name};

fn main() {
    for path in std::env::args().skip(1) {
        let (samples, rate) = wav::load(&path);
        let mut detector = NoteDetector::new(rate);
        let mut notes = Vec::new();
        // Feed it in blocks, as a live stream would arrive.
        for block in samples.chunks(256) {
            detector.push(block, &mut notes);
        }
        detector.flush(&mut notes);

        println!("{path}: {} notes", notes.len());
        for n in notes {
            let cents = (n.midi - n.midi.round()) * 100.0;
            println!(
                "  {:7.3}s  {:>4} {cents:+4.0}c  step {:+5.1} dB",
                n.time,
                note_name(n.midi),
                n.step_db
            );
        }
    }
}
