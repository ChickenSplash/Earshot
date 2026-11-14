//! Microphone input. Each frame, `Mic::samples` holds the newest audio, mono.
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
}

pub struct MicPlugin;

impl Plugin for MicPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Mic {
            status: MicStatus::Starting,
            sample_rate: 48_000.0,
            samples: vec![0.0; WINDOW],
            fresh: false,
        })
        .add_systems(Startup, backend::start)
        .add_systems(PreUpdate, backend::read);
    }
}

impl Mic {
    /// Append new audio, keeping only the newest `WINDOW` samples.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    fn push(&mut self, new: &[f32]) {
        if new.is_empty() {
            return;
        }
        self.samples.extend_from_slice(new);
        let excess = self.samples.len().saturating_sub(WINDOW);
        self.samples.drain(..excess);
        self.fresh = true;
    }
}
