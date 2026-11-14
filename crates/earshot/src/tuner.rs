//! Guitar tuner screen: pluck one string and it shows the nearest string in
//! standard tuning and how many cents sharp or flat you are.

use bevy::prelude::*;
use bevy::text::FontSize;
use earshot_dsp::{STANDARD_TUNING, Tuner, Yin, level_db, midi_to_hz, note_name};

use crate::mic::{Mic, MicStatus};

/// Quieter than this is treated as silence.
const GATE_DB: f32 = -50.0;
const MIN_HZ: f32 = 60.0;
const MAX_HZ: f32 = 1000.0;
/// The needle's full travel either side of centre.
const NEEDLE_RANGE_CENTS: f32 = 50.0;

const TEXT: Color = Color::srgb(0.92, 0.92, 0.95);
const DIM: Color = Color::srgb(0.45, 0.45, 0.52);
const TRACK: Color = Color::srgb(0.18, 0.18, 0.22);
const GOOD: Color = Color::srgb(0.35, 0.85, 0.5);
const OFF: Color = Color::srgb(0.95, 0.6, 0.25);

pub struct TunerPlugin;

impl Plugin for TunerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, (listen, show).chain());
    }
}

#[derive(Resource)]
struct Listening {
    yin: Yin,
    tuner: Tuner,
    level: f32,
    reading: Option<earshot_dsp::Reading>,
}

#[derive(Component)]
struct NoteText;
#[derive(Component)]
struct DetailText;
#[derive(Component)]
struct Needle;
#[derive(Component)]
struct StringLabel(usize);
#[derive(Component)]
struct LevelBar;
#[derive(Component)]
struct StatusText;

fn listen(
    mic: Res<Mic>,
    time: Res<Time>,
    mut listening: Option<ResMut<Listening>>,
    mut commands: Commands,
) {
    if !mic.fresh {
        return;
    }
    let Some(state) = listening
        .as_deref_mut()
        .filter(|l| l.yin.sample_rate() == mic.sample_rate)
    else {
        commands.insert_resource(Listening {
            yin: Yin::new(mic.sample_rate, MIN_HZ, MAX_HZ),
            tuner: Tuner::new(&STANDARD_TUNING),
            level: f32::NEG_INFINITY,
            reading: None,
        });
        return;
    };

    state.level = level_db(&mic.samples);
    let hz = if state.level > GATE_DB {
        state.yin.detect(&mic.samples).map(|p| p.hz)
    } else {
        None
    };
    state.reading = state.tuner.update(hz, time.elapsed_secs_f64());
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands
        .spawn(Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(24),
            ..default()
        })
        .with_children(|ui| {
            ui.spawn((Text::new("Tuner"), font(28.0), TextColor(DIM)));
            ui.spawn((Text::new("·"), font(160.0), TextColor(TEXT), NoteText));

            // Needle track: centre is in tune, left is flat, right is sharp.
            ui.spawn((
                Node {
                    width: px(600),
                    height: px(10),
                    ..default()
                },
                BackgroundColor(TRACK),
            ))
            .with_children(|track| {
                track.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(50),
                        top: px(-12),
                        width: px(2),
                        height: px(34),
                        ..default()
                    },
                    BackgroundColor(DIM),
                ));
                track.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(50),
                        top: px(-22),
                        width: px(6),
                        height: px(54),
                        margin: UiRect::left(px(-3)),
                        ..default()
                    },
                    BackgroundColor(TEXT),
                    Needle,
                ));
            });

            ui.spawn((Text::new(""), font(24.0), TextColor(TEXT), DetailText));

            ui.spawn(Node {
                column_gap: px(28),
                ..default()
            })
            .with_children(|row| {
                for (i, midi) in STANDARD_TUNING.into_iter().enumerate() {
                    row.spawn((
                        Text::new(note_name(midi as f32)),
                        font(30.0),
                        TextColor(DIM),
                        StringLabel(i),
                    ));
                }
            });

            ui.spawn((
                Node {
                    width: px(300),
                    height: px(6),
                    ..default()
                },
                BackgroundColor(TRACK),
            ))
            .with_child((
                Node {
                    width: percent(0),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(DIM),
                LevelBar,
            ));

            ui.spawn((Text::new(""), font(18.0), TextColor(DIM), StatusText));
        });
}

fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show(
    mic: Res<Mic>,
    listening: Option<Res<Listening>>,
    mut note: Single<&mut Text, (With<NoteText>, Without<DetailText>, Without<StatusText>)>,
    mut detail: Single<
        (&mut Text, &mut TextColor),
        (With<DetailText>, Without<NoteText>, Without<StatusText>),
    >,
    mut status: Single<&mut Text, (With<StatusText>, Without<NoteText>, Without<DetailText>)>,
    mut needle: Single<(&mut Node, &mut BackgroundColor), (With<Needle>, Without<LevelBar>)>,
    mut level_bar: Single<(&mut Node, &mut BackgroundColor), (With<LevelBar>, Without<Needle>)>,
    mut strings: Query<(&StringLabel, &mut TextColor), Without<DetailText>>,
) {
    let message = match &mic.status {
        MicStatus::Starting => "Opening the mic…".to_owned(),
        MicStatus::WaitingForClick => "Click anywhere to turn on the mic".to_owned(),
        MicStatus::Listening { device } => {
            format!("Listening on {device} · pluck one string and let it ring")
        }
        MicStatus::Failed(err) => format!("Couldn't open the mic: {err}"),
    };
    if status.0 != message {
        status.0 = message;
    }

    let (level, reading) = listening.map_or((f32::NEG_INFINITY, None), |l| (l.level, l.reading));

    // -70 dB to -10 dB fills the meter.
    let (bar_node, bar_color) = &mut *level_bar;
    bar_node.width = percent(((level + 70.0) / 60.0).clamp(0.0, 1.0) * 100.0);
    bar_color.0 = if level > GATE_DB { TEXT } else { DIM };

    let (needle_node, needle_color) = &mut *needle;
    let (detail_text, detail_color) = &mut *detail;
    match reading {
        Some(r) => {
            note.0 = note_name(STANDARD_TUNING[r.string] as f32);
            let travel = (r.cents / NEEDLE_RANGE_CENTS).clamp(-1.0, 1.0);
            needle_node.left = percent(50.0 + travel * 50.0);
            let colour = if r.in_tune { GOOD } else { OFF };
            needle_color.0 = colour;
            detail_color.0 = colour;
            let cents = r.cents.round() as i32;
            detail_text.0 = if r.in_tune {
                format!("In tune · {:.1} Hz", midi_to_hz(r.midi))
            } else {
                let direction = if cents > 0 { "down" } else { "up" };
                format!(
                    "{cents:+} cents · tune {direction} · {:.1} Hz",
                    midi_to_hz(r.midi)
                )
            };
        }
        None => {
            note.0 = "·".into();
            needle_node.left = percent(50);
            needle_color.0 = TEXT;
            detail_text.0.clear();
        }
    }

    for (label, mut colour) in &mut strings {
        colour.0 = match reading {
            Some(r) if r.string == label.0 && r.in_tune => GOOD,
            Some(r) if r.string == label.0 => TEXT,
            _ => DIM,
        };
    }
}
