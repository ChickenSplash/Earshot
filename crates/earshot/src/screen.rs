//! Which screen is showing, and the bar at the top for switching. Tab also
//! switches to the next screen, and a screen's name as the first argument
//! (`earshot monitor`) starts on it.

use bevy::prelude::*;

use crate::ui::{DIM, TEXT, font};

#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    Tuner,
    Monitor,
}

const SCREENS: [(Screen, &str); 2] = [(Screen::Tuner, "Tuner"), (Screen::Monitor, "Monitor")];

pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Screen>()
            .add_systems(Startup, setup)
            .add_systems(Update, (tab, highlight.run_if(state_changed::<Screen>)));
    }
}

#[derive(Component)]
struct Tab(Screen);

fn setup(mut commands: Commands, mut next: ResMut<NextState<Screen>>) {
    if let Some(wanted) = std::env::args().nth(1)
        && let Some((screen, _)) = SCREENS
            .iter()
            .find(|(_, name)| name.eq_ignore_ascii_case(&wanted))
    {
        next.set(*screen);
    }

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: px(20),
            width: percent(100),
            justify_content: JustifyContent::Center,
            column_gap: px(32),
            ..default()
        })
        .with_children(|bar| {
            for (screen, name) in SCREENS {
                bar.spawn((Text::new(name), font(22.0), TextColor(DIM), Tab(screen)))
                    .observe(
                        move |_: On<PointerClick>, mut next: ResMut<NextState<Screen>>| {
                            next.set(screen);
                        },
                    );
            }
        });
}

fn tab(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<State<Screen>>,
    mut next: ResMut<NextState<Screen>>,
) {
    if keys.just_pressed(KeyCode::Tab) {
        let at = SCREENS.iter().position(|(s, _)| s == screen.get()).unwrap();
        next.set(SCREENS[(at + 1) % SCREENS.len()].0);
    }
}

fn highlight(screen: Res<State<Screen>>, mut tabs: Query<(&Tab, &mut TextColor)>) {
    for (tab, mut colour) in &mut tabs {
        colour.0 = if tab.0 == *screen.get() { TEXT } else { DIM };
    }
}
