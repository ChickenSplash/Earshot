//! Turns a stream of audio into note events: when each note was plucked, and
//! which note it was.
//!
//! 1. **Candidate pluck:** a peak in spectral flux, which measures how much new
//!    energy appeared across all frequencies since the last frame.
//! 2. **Confirm it:** a pluck adds energy that lasts, so the level just after
//!    must be higher than just before. Finger releases, muting and most pick
//!    clicks fail this.
//! 3. **Time it:** the flux peak can be the pick touching the string, so the
//!    pluck time is moved to where the level actually rises.
//! 4. **Pitch:** the median of several YIN readings, starting just after the
//!    pick attack (strings ring sharp for a moment) and using a short window so
//!    the previous note doesn't leak in. No steady pitch means no note, which
//!    filters out handling noise.
//! 5. **Last checks:** re-plucking the note already ringing needs a clear level
//!    jump, and a weak pluck much quieter than recent playing is ignored.
//!
//! The thresholds were tuned on the recordings in `clips/`, see `tests/clips.rs`.
//! Events come out about 150 ms after the pluck, stamped with the pluck time.

use std::collections::VecDeque;
use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};

use crate::{Yin, hz_to_midi, level_db};

/// Spectral flux frame, about 21 ms at 48 kHz.
const FRAME: usize = 1024;
/// Window for pitch readings: just over two periods of the lowest note.
const PITCH_WINDOW: usize = 2048;
const HOP_SECS: f64 = 0.005;

/// Flux peaks are compared with the median of this much recent flux.
const FLUX_HISTORY: usize = 60;
const FLUX_RATIO: f32 = 1.5;
const FLUX_FLOOR: f32 = 0.02;
/// Hops either side a flux peak must beat, which is also how long we wait to confirm it.
const PEAK_HOPS: usize = 3;
/// Two plucks closer than this are counted as one.
const MIN_GAP_SECS: f64 = 0.075;
/// Unless the second is at least this far after the first and much stronger:
/// then it is the real pluck, and the first was the fretting finger landing.
/// (Closer, stronger peaks are the same attack spreading through the frame.)
const REANCHOR_SECS: f64 = 0.040;
const REANCHOR_RATIO: f32 = 1.5;

/// The level is compared over these spans before and after the candidate pluck.
const BEFORE_SECS: (f64, f64) = (-0.060, -0.010);
const AFTER_SECS: (f64, f64) = (0.020, 0.080);
/// The level must rise by at least this much for a pluck to count.
const MIN_STEP_DB: f32 = 1.5;
/// And end up louder than this.
const GATE_DB: f32 = -55.0;
/// Where to look for the real start of the sound around a flux peak, and the
/// block length the level is measured over while looking.
const ATTACK_SEARCH_SECS: (f64, f64) = (-0.010, 0.075);
const ATTACK_BLOCK_SECS: f64 = 0.005;
/// The sound has started once the level is halfway (in dB) from before to after,
/// but never more than this below after: on a pluck from near silence, halfway
/// can fall inside the faint noise of the pick touching the string.
const ATTACK_BELOW_AFTER_DB: f32 = 6.0;

/// Pitch readings start this long after the pluck, past the pick attack...
const SETTLE_SECS: f64 = 0.020;
/// ...and stop this long after it, when the note is reported.
const LISTEN_SECS: f64 = 0.150;
/// Readings that must agree (within a semitone of the median) for a note.
const MIN_READINGS: usize = 4;
/// The note's pitch must still be heard this close to the end of listening.
const STILL_RINGING_SECS: f64 = 0.015;
/// Re-plucking the note that is already ringing must raise the level by at
/// least this much. Finger releases and double triggers barely change it.
const SAME_NOTE_MIN_STEP_DB: f32 = 3.0;
/// How long a note is assumed to keep ringing, for the same-note rule.
const RING_SECS: f64 = 3.0;
/// A pluck that raised the level by less than this is weak evidence on its own...
const WEAK_STEP_DB: f32 = 6.0;
/// ...so it must also be within this much of the loudest note in the last few
/// seconds. Faint leftovers after a finger squeak are far quieter than playing.
const WEAK_MAX_BELOW_DB: f32 = 12.0;
const RECENT_SECS: f64 = 5.0;

/// A plucked note.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteEvent {
    /// Seconds from the start of the stream to the pluck.
    pub time: f64,
    /// Fractional MIDI note, the median of the readings.
    pub midi: f32,
    /// How much louder it got, in dB. Bigger means a firmer pluck.
    pub step_db: f32,
}

struct Pending {
    /// Absolute sample index of the pluck.
    onset: u64,
    flux: f32,
    /// Level rise over the pluck, and the level just after it, once measured.
    step_db: Option<f32>,
    after_db: f32,
    /// MIDI note of each reading, with the sample index its window ended at.
    readings: Vec<(f32, u64)>,
}

impl Pending {
    fn new(onset: u64, flux: f32) -> Self {
        Self {
            onset,
            flux,
            step_db: None,
            after_db: f32::NEG_INFINITY,
            readings: Vec::new(),
        }
    }
}

pub struct NoteDetector {
    rate: f64,
    hop: u64,
    /// Recent audio. `buf[0]` is absolute sample `buf_start`.
    buf: Vec<f32>,
    buf_start: u64,
    /// Absolute sample index where the next flux frame ends.
    next_frame_end: u64,

    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    fft_in: Vec<f32>,
    fft_out: Vec<Complex<f32>>,
    prev_spectrum: Option<Vec<f32>>,
    /// (flux, absolute sample at the frame's centre), newest last.
    flux: VecDeque<(f32, u64)>,
    last_onset: Option<u64>,

    yin: Yin,
    pending: Option<Pending>,
    last_note: Option<NoteEvent>,
    /// (time, level just after the pluck) of recent notes, oldest first.
    recent: VecDeque<(f64, f32)>,
}

impl NoteDetector {
    pub fn new(sample_rate: f32) -> Self {
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(FRAME);
        let window = (0..FRAME)
            .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FRAME as f32).cos())
            .collect();
        Self {
            rate: sample_rate as f64,
            hop: (sample_rate as f64 * HOP_SECS).round() as u64,
            buf: Vec::new(),
            buf_start: 0,
            next_frame_end: FRAME as u64,
            fft_in: fft.make_input_vec(),
            fft_out: fft.make_output_vec(),
            fft,
            window,
            prev_spectrum: None,
            flux: VecDeque::with_capacity(FLUX_HISTORY + PEAK_HOPS * 2 + 1),
            last_onset: None,
            yin: Yin::new(sample_rate, 60.0, 1000.0),
            pending: None,
            last_note: None,
            recent: VecDeque::new(),
        }
    }

    /// Feed the next block of mono audio. Notes that finished are appended to `out`.
    pub fn push(&mut self, samples: &[f32], out: &mut Vec<NoteEvent>) {
        self.buf.extend_from_slice(samples);
        while self.next_frame_end <= self.end() {
            self.hop_at(self.next_frame_end, out);
            self.next_frame_end += self.hop;
        }
        // Keep enough history to look back from a pluck that is still being confirmed.
        let keep = (self.rate * 0.5) as usize;
        if self.buf.len() > keep * 2 {
            let drop = self.buf.len() - keep;
            self.buf.drain(..drop);
            self.buf_start += drop as u64;
        }
    }

    /// Report a note still being listened to, e.g. when the stream ends.
    pub fn flush(&mut self, out: &mut Vec<NoteEvent>) {
        if let Some(pending) = self.pending.take() {
            self.finish(pending, self.end(), out);
        }
    }

    fn end(&self) -> u64 {
        self.buf_start + self.buf.len() as u64
    }

    /// Samples from absolute index `from` to `to`, clipped to what is buffered.
    fn span(&self, from: i64, to: i64) -> &[f32] {
        let clip = |i: i64| (i - self.buf_start as i64).clamp(0, self.buf.len() as i64) as usize;
        &self.buf[clip(from)..clip(to)]
    }

    fn secs(&self, secs: f64) -> i64 {
        (secs * self.rate).round() as i64
    }

    fn hop_at(&mut self, end: u64, out: &mut Vec<NoteEvent>) {
        let flux = self.spectral_flux(end);
        self.flux.push_back((flux, end - FRAME as u64 / 2));
        if self.flux.len() > FLUX_HISTORY + PEAK_HOPS * 2 + 1 {
            self.flux.pop_front();
        }

        if let Some((onset, flux, spaced)) = self.peak() {
            if spaced {
                if let Some(pending) = self.pending.take() {
                    self.finish(pending, end, out);
                }
                self.pending = Some(Pending::new(onset, flux));
                self.last_onset = Some(onset);
            } else {
                let min_gap = self.secs(REANCHOR_SECS);
                if let Some(pending) = self.pending.as_mut()
                    && onset as i64 - pending.onset as i64 >= min_gap
                    && flux > pending.flux * REANCHOR_RATIO
                {
                    *pending = Pending::new(onset, flux);
                    self.last_onset = Some(onset);
                }
            }
        }

        let Some(mut pending) = self.pending.take() else {
            return;
        };
        let since = end as i64 - pending.onset as i64;

        if pending.step_db.is_none() && since >= self.secs(AFTER_SECS.1) {
            let at = |s: f64| pending.onset as i64 + self.secs(s);
            let before = level_db(self.span(at(BEFORE_SECS.0), at(BEFORE_SECS.1)));
            let after = level_db(self.span(at(AFTER_SECS.0), at(AFTER_SECS.1)));
            if after < GATE_DB || after - before < MIN_STEP_DB {
                return;
            }
            pending.step_db = Some(after - before);
            pending.after_db = after;
            let threshold = ((before + after) / 2.0).max(after - ATTACK_BELOW_AFTER_DB);
            pending.onset = self.attack(pending.onset, threshold);
        }

        let window_start = end as i64 - PITCH_WINDOW as i64;
        if window_start >= pending.onset as i64 + self.secs(SETTLE_SECS) {
            let window = self.span(window_start, end as i64).to_vec();
            if let Some(pitch) = self.yin.detect(&window) {
                pending.readings.push((hz_to_midi(pitch.hz), end));
            }
        }

        if since >= self.secs(LISTEN_SECS) {
            self.finish(pending, end, out);
        } else {
            self.pending = Some(pending);
        }
    }

    /// When the sound really starts. The flux peak can come from the pick
    /// touching the string, up to ~60 ms before it lets go, so find where the
    /// level first reaches `threshold_db`.
    fn attack(&self, peak: u64, threshold_db: f32) -> u64 {
        let (block, step) = (self.secs(ATTACK_BLOCK_SECS), self.secs(0.001));
        let (from, to) = (
            self.secs(ATTACK_SEARCH_SECS.0),
            self.secs(ATTACK_SEARCH_SECS.1),
        );
        (from..=to)
            .step_by(step as usize)
            .map(|offset| peak as i64 + offset)
            .find(|&t| level_db(self.span(t - block, t)) >= threshold_db)
            .map_or(peak, |t| t.max(0) as u64)
    }

    /// Positive change in log-magnitude spectrum since the previous frame.
    fn spectral_flux(&mut self, end: u64) -> f32 {
        let start = (end - self.buf_start) as usize - FRAME;
        let frame = &self.buf[start..start + FRAME];
        for ((dst, src), w) in self.fft_in.iter_mut().zip(frame).zip(&self.window) {
            *dst = src * w;
        }
        self.fft
            .process(&mut self.fft_in, &mut self.fft_out)
            .expect("fft sizes match");
        // log1p compresses loud and quiet alike, so a pluck over a ringing note still shows.
        let spectrum: Vec<f32> = self
            .fft_out
            .iter()
            .map(|c| (1000.0 * c.norm()).ln_1p())
            .collect();
        let flux = match &self.prev_spectrum {
            Some(prev) => {
                spectrum
                    .iter()
                    .zip(prev)
                    .map(|(m, p)| (m - p).max(0.0))
                    .sum::<f32>()
                    / spectrum.len() as f32
            }
            None => 0.0,
        };
        self.prev_spectrum = Some(spectrum);
        flux
    }

    /// The frame `PEAK_HOPS` ago, if it is a flux peak: the pluck's sample index,
    /// the flux, and whether it is far enough from the last pluck to be a new one.
    fn peak(&self) -> Option<(u64, f32, bool)> {
        let n = self.flux.len();
        if n < PEAK_HOPS * 2 + 2 {
            return None;
        }
        let at = n - 1 - PEAK_HOPS;
        let (value, centre) = self.flux[at];
        let neighbours = self.flux.range(at - PEAK_HOPS..=at + PEAK_HOPS);
        if neighbours.clone().any(|&(f, _)| f > value) {
            return None;
        }
        let mut history: Vec<f32> = self.flux.range(..at).map(|&(f, _)| f).collect();
        history.sort_by(f32::total_cmp);
        let threshold = history[history.len() / 2] * FLUX_RATIO + FLUX_FLOOR;
        let spaced = self
            .last_onset
            .is_none_or(|last| centre - last >= self.secs(MIN_GAP_SECS) as u64);
        (value > threshold).then_some((centre, value, spaced))
    }

    /// Turn a pluck into a note if it passes every check. `end` is when listening stopped.
    fn finish(&mut self, pending: Pending, end: u64, out: &mut Vec<NoteEvent>) {
        let Some(step_db) = pending.step_db else {
            return;
        };
        let mut sorted: Vec<f32> = pending.readings.iter().map(|&(m, _)| m).collect();
        sorted.sort_by(f32::total_cmp);
        let Some(&midi) = sorted.get(sorted.len() / 2) else {
            return;
        };
        let agreeing = sorted.iter().filter(|r| (*r - midi).abs() < 1.0).count();
        let ringing = pending.readings.last().is_some_and(|&(last, at)| {
            (last - midi).abs() < 1.0 && end - at <= self.secs(STILL_RINGING_SECS) as u64
        });
        if agreeing < MIN_READINGS || !ringing {
            return;
        }

        let note = NoteEvent {
            time: pending.onset as f64 / self.rate,
            midi,
            step_db,
        };
        let replucked = self.last_note.is_some_and(|last| {
            (last.midi - midi).abs() < 0.5 && note.time - last.time < RING_SECS
        });
        if replucked && step_db < SAME_NOTE_MIN_STEP_DB {
            return;
        }
        while self
            .recent
            .front()
            .is_some_and(|&(t, _)| note.time - t > RECENT_SECS)
        {
            self.recent.pop_front();
        }
        let loudest = self
            .recent
            .iter()
            .map(|&(_, db)| db)
            .fold(f32::NEG_INFINITY, f32::max);
        if step_db < WEAK_STEP_DB && pending.after_db < loudest - WEAK_MAX_BELOW_DB {
            return;
        }
        self.recent.push_back((note.time, pending.after_db));
        self.last_note = Some(note);
        out.push(note);
    }
}
