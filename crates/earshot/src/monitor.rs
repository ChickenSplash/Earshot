//! Note monitor screen: shows each note as it's plucked, to check what the
//! note detector hears.

use bevy::prelude::*;
use earshot_dsp::note_name;

use crate::mic::{Mic, STREAMS};
use crate::notes::{Notes, Plucked};
use crate::screen::Screen;
use crate::ui::{DIM, GOOD, TEXT, font, screen_root};

/// Earlier notes shown in the row under the latest one.
const HISTORY: usize = 8;
/// How long the latest note glows after it's heard.
const FLASH_SECS: f32 = 0.3;

pub struct MonitorPlugin;

impl Plugin for MonitorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Monitor), setup)
            .add_systems(
                Update,
                show.after(crate::notes::detect)
                    .run_if(in_state(Screen::Monitor)),
            );
    }
}

#[derive(Component)]
struct NoteText;
#[derive(Component)]
struct DetailText;
#[derive(Component)]
struct HistoryText;
#[derive(Component)]
struct StatusText;

fn setup(mut commands: Commands) {
    commands
        .spawn(screen_root(Screen::Monitor))
        .with_children(|ui| {
            ui.spawn((Text::new("·"), font(160.0), TextColor(TEXT), NoteText));
            ui.spawn((Text::new(""), font(24.0), TextColor(DIM), DetailText));
            ui.spawn((Text::new(""), font(30.0), TextColor(DIM), HistoryText));
            ui.spawn((Text::new(""), font(18.0), TextColor(DIM), StatusText));
        });
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show(
    mic: Res<Mic>,
    notes: Res<Notes>,
    time: Res<Time>,
    mut plucked: MessageReader<Plucked>,
    mut flashed_at: Local<Option<f32>>,
    mut note: Single<(&mut Text, &mut TextColor), With<NoteText>>,
    mut detail: Single<&mut Text, (With<DetailText>, Without<NoteText>)>,
    mut history: Single<&mut Text, (With<HistoryText>, Without<NoteText>, Without<DetailText>)>,
    mut status: Single<
        &mut Text,
        (
            With<StatusText>,
            Without<NoteText>,
            Without<DetailText>,
            Without<HistoryText>,
        ),
    >,
) {
    let now = time.elapsed_secs();
    if plucked.read().count() > 0 {
        *flashed_at = Some(now);
    }

    let message = if STREAMS {
        mic.status_line("pluck any note")
    } else {
        "The note monitor needs the desktop build for now".to_owned()
    };
    if status.0 != message {
        status.0 = message;
    }

    let (note_text, note_colour) = &mut *note;
    let glow = flashed_at.map_or(0.0, |at| 1.0 - ((now - at) / FLASH_SECS).clamp(0.0, 1.0));
    note_colour.0 = TEXT.mix(&GOOD, glow);

    let mut recent = notes.recent.iter().rev();
    let Some(latest) = recent.next() else {
        return;
    };
    note_text.0 = note_name(latest.midi);
    let cents = ((latest.midi - latest.midi.round()) * 100.0).round();
    detail.0 = match notes.recent.iter().rev().nth(1) {
        Some(previous) => format!(
            "{cents:+} cents · {:.2} s after the last note",
            latest.time - previous.time
        ),
        None => format!("{cents:+} cents"),
    };
    let mut earlier: Vec<_> = recent.take(HISTORY).map(|n| note_name(n.midi)).collect();
    earlier.reverse();
    history.0 = earlier.join("   ");
}
