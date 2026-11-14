//! Microphone input. Each frame, `Mic::samples` holds the newest audio, mono,
//! and `Mic::stream` the audio that arrived since the last frame.
//!
//! Native builds capture through cpal (PipeWire on Linux). Browsers need a
//! click before they allow audio, so the web build waits for one.

use bevy::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
use native as backend;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
use web as backend;

/// Samples kept for analysis: about 85 ms at 48 kHz, enough for low E.
pub const WINDOW: usize = 4096;

/// Whether `Mic::stream` gets audio. The browser build only sees a window of
/// the newest audio each frame, which isn't enough to time notes.
pub const STREAMS: bool = cfg!(not(target_arch = "wasm32"));

#[derive(Clone, Debug, PartialEq)]
pub enum MicStatus {
    Starting,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    WaitingForClick,
    Listening {
        device: String,
    },
    Failed(String),
}

#[derive(Resource)]
pub struct Mic {
    pub status: MicStatus,
    pub sample_rate: f32,
    /// The newest `WINDOW` samples, oldest first.
    pub samples: Vec<f32>,
    /// Whether new audio arrived this frame.
    pub fresh: bool,
    /// Every sample that arrived this frame, in order. Empty if not `STREAMS`.
    pub stream: Vec<f32>,
}

pub struct MicPlugin;

impl Plugin for MicPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Mic {
            status: MicStatus::Starting,
            sample_rate: 48_000.0,
            samples: vec![0.0; WINDOW],
            fresh: false,
            stream: Vec::new(),
        })
        .add_systems(Startup, backend::start)
        .add_systems(PreUpdate, backend::read);
    }
}

impl Mic {
    /// One line on what the mic is doing, ending with `hint` once it's listening.
    pub fn status_line(&self, hint: &str) -> String {
        match &self.status {
            MicStatus::Starting => "Opening the mic…".to_owned(),
            MicStatus::WaitingForClick => "Click anywhere to turn on the mic".to_owned(),
            MicStatus::Listening { device } => format!("Listening on {device} · {hint}"),
            MicStatus::Failed(err) => format!("Couldn't open the mic: {err}"),
        }
    }

    /// Append new audio, keeping only the newest `WINDOW` samples.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    fn push(&mut self, new: &[f32]) {
        if new.is_empty() {
            return;
        }
        self.stream.extend_from_slice(new);
        self.samples.extend_from_slice(new);
        let excess = self.samples.len().saturating_sub(WINDOW);
        self.samples.drain(..excess);
        self.fresh = true;
    }
}
