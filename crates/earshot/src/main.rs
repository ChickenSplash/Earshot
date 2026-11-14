mod mic;
mod tuner;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Earshot".into(),
                // Wayland app id / X11 class, for window rules.
                name: Some("earshot".into()),
                // On the web, draw into the page's canvas and follow its size.
                canvas: Some("#earshot".into()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.07, 0.07, 0.09)))
        .add_plugins((mic::MicPlugin, tuner::TunerPlugin))
        .add_systems(PreStartup, use_fira_sans)
        .run();
}

/// Bevy's built-in font is a small subset with no `·`, so replace the default
/// font with Fira Sans. Embedded so native and web builds need no asset files.
fn use_fira_sans(mut fonts: ResMut<Assets<Font>>) {
    let font = Font::from_bytes(include_bytes!("../assets/fonts/FiraSans-Regular.ttf").to_vec());
    let _ = fonts.insert(&Handle::<Font>::default(), font);
}
