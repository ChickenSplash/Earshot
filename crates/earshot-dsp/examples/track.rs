//! Prints what the pitch detector hears in a WAV file, as a timeline.
//!
//! cargo run -p earshot-dsp --example track -- clips/open-e2.wav [step_ms]

#[path = "../support/wav.rs"]
mod wav;

use earshot_dsp::{Yin, hz_to_midi, level_db, note_name};

const WINDOW: usize = 4096;
const GATE_DB: f32 = -50.0;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: track <file.wav> [step_ms]");
    let step_ms: f32 = args
        .next()
        .map_or(50.0, |s| s.parse().expect("step_ms must be a number"));

    let (samples, rate) = wav::load(&path);
    let step = (rate * step_ms / 1000.0) as usize;
    let mut yin = Yin::new(rate, 60.0, 1000.0);

    println!("{path}: {:.2} s at {rate} Hz", samples.len() as f32 / rate);
    let mut start = 0;
    while start + WINDOW <= samples.len() {
        let window = &samples[start..start + WINDOW];
        let level = level_db(window);
        // Time of the window's newest sample, as the live app would see it.
        let time = (start + WINDOW) as f32 / rate;
        let reading = match yin.detect(window) {
            Some(p) if level > GATE_DB => {
                let midi = hz_to_midi(p.hz);
                let cents = (midi - midi.round()) * 100.0;
                format!(
                    "{:>4} {cents:+4.0}c {:7.2} Hz  clarity {:.2}",
                    note_name(midi),
                    p.hz,
                    p.clarity
                )
            }
            Some(_) => "(below gate)".into(),
            None => String::new(),
        };
        println!("{time:7.2}  {level:6.1} dB  {reading}");
        start += step;
    }
}
