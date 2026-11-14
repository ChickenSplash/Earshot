//! Browser capture through Web Audio. An `AnalyserNode` always holds the
//! newest audio, so each frame copies its whole window rather than streaming.
//! Good for the tuner; note-timing games will want an AudioWorklet later.

use std::cell::RefCell;

use bevy::prelude::*;
use js_sys::{Object, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AnalyserNode, AudioContext, MediaStream, MediaStreamConstraints};

use super::{Mic, MicStatus, WINDOW};

struct WebMic {
    // Kept alive so the browser keeps capturing.
    _context: AudioContext,
    analyser: AnalyserNode,
}

enum State {
    Idle,
    Opening,
    Open(WebMic),
    Failed(String),
}

// JS objects can't cross threads, and the browser build is single-threaded,
// so the capture lives here rather than in a Bevy resource.
thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::Idle) };
}

pub(super) fn start(mut mic: ResMut<Mic>) {
    mic.status = MicStatus::WaitingForClick;
}

pub(super) fn read(
    mut mic: ResMut<Mic>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    mic.fresh = false;
    STATE.with_borrow_mut(|state| match state {
        State::Idle
            if mouse.get_just_pressed().next().is_some()
                || keys.get_just_pressed().next().is_some() =>
        {
            *state = State::Opening;
            mic.status = MicStatus::Starting;
            wasm_bindgen_futures::spawn_local(async {
                let opened = open().await.map_err(describe);
                STATE.with_borrow_mut(|state| {
                    *state = match opened {
                        Ok(web_mic) => State::Open(web_mic),
                        Err(err) => State::Failed(err),
                    }
                });
            });
        }
        State::Open(web_mic) => {
            if !matches!(mic.status, MicStatus::Listening { .. }) {
                mic.sample_rate = web_mic.analyser.context().sample_rate();
                mic.status = MicStatus::Listening {
                    device: "browser microphone".into(),
                };
            }
            web_mic
                .analyser
                .get_float_time_domain_data(&mut mic.samples);
            mic.fresh = true;
        }
        State::Failed(err) if !matches!(mic.status, MicStatus::Failed(_)) => {
            mic.status = MicStatus::Failed(err.clone());
        }
        _ => {}
    });
}

async fn open() -> Result<WebMic, JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let devices = window.navigator().media_devices()?;

    // Browsers clean up voice for calls by default, which mangles a guitar.
    let audio = Object::new();
    for setting in ["echoCancellation", "noiseSuppression", "autoGainControl"] {
        Reflect::set(&audio, &setting.into(), &false.into())?;
    }
    let constraints = MediaStreamConstraints::new();
    constraints.set_audio(&audio);

    let stream: MediaStream =
        JsFuture::from(devices.get_user_media_with_constraints(&constraints)?)
            .await?
            .dyn_into()?;

    let context = AudioContext::new()?;
    let _ = context.resume();
    let source = context.create_media_stream_source(&stream)?;
    let analyser = context.create_analyser()?;
    analyser.set_fft_size(WINDOW as u32);
    source.connect_with_audio_node(&analyser)?;
    Ok(WebMic {
        _context: context,
        analyser,
    })
}

fn describe(err: JsValue) -> String {
    err.dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .or_else(|| err.as_string())
        .unwrap_or_else(|| format!("{err:?}"))
}
