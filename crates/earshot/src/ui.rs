//! Colours and text styles shared by the screens.

use bevy::prelude::*;
use bevy::text::FontSize;

pub const TEXT: Color = Color::srgb(0.92, 0.92, 0.95);
pub const DIM: Color = Color::srgb(0.45, 0.45, 0.52);
pub const TRACK: Color = Color::srgb(0.18, 0.18, 0.22);
pub const GOOD: Color = Color::srgb(0.35, 0.85, 0.5);
pub const OFF: Color = Color::srgb(0.95, 0.6, 0.25);

pub fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

/// A full-window column, centred, removed when leaving `screen`.
pub fn screen_root<S: States>(screen: S) -> impl Bundle {
    (
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(24),
            ..default()
        },
        DespawnOnExit(screen),
    )
}
