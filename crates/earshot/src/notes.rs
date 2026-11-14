//! Runs the note detector on the live mic stream, whichever screen is showing.

use std::collections::VecDeque;

use bevy::prelude::*;
use earshot_dsp::{NoteDetector, NoteEvent};

use crate::mic::Mic;

/// Notes kept for screens that show recent playing.
const KEEP: usize = 16;

/// A note was just heard. Arrives about 150 ms after the pluck.
#[derive(Message, Clone, Copy, Debug)]
pub struct Plucked(pub NoteEvent);

#[derive(Resource, Default)]
pub struct Notes {
    detector: Option<(NoteDetector, f32)>,
    found: Vec<NoteEvent>,
    /// Recent notes, oldest first.
    pub recent: VecDeque<NoteEvent>,
}

pub struct NotesPlugin;

impl Plugin for NotesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Notes>()
            .add_message::<Plucked>()
            .add_systems(Update, detect);
    }
}

pub fn detect(mic: Res<Mic>, mut notes: ResMut<Notes>, mut plucked: MessageWriter<Plucked>) {
    if mic.stream.is_empty() {
        return;
    }
    let notes = &mut *notes;
    if notes
        .detector
        .as_ref()
        .is_none_or(|(_, rate)| *rate != mic.sample_rate)
    {
        notes.detector = Some((NoteDetector::new(mic.sample_rate), mic.sample_rate));
    }
    let (detector, _) = notes.detector.as_mut().unwrap();
    detector.push(&mic.stream, &mut notes.found);

    for note in notes.found.drain(..) {
        plucked.write(Plucked(note));
        notes.recent.push_back(note);
        if notes.recent.len() > KEEP {
            notes.recent.pop_front();
        }
    }
}
