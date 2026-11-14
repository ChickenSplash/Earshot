//! Native capture with cpal. The audio callback runs on a real-time thread, so
//! it only mixes to mono and pushes into a lock-free ring buffer; all the
//! analysis happens on Bevy's side.

use std::sync::{Mutex, mpsc};
use std::thread;

use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use rtrb::{Consumer, Producer, RingBuffer};

use super::{Mic, MicStatus};

/// Set to part of an input device's name to use it instead of the system default.
const DEVICE_ENV: &str = "EARSHOT_INPUT";

type Opened = Result<(String, u32), String>;

#[derive(Resource)]
pub(super) struct Capture {
    // Mutex only because Bevy resources must be Sync; nothing else touches these.
    samples: Mutex<Consumer<f32>>,
    opened: Mutex<Option<mpsc::Receiver<Opened>>>,
}

pub(super) fn start(mut commands: Commands) {
    // One second of audio: plenty of slack if a frame is slow.
    let (producer, consumer) = RingBuffer::new(48_000);
    let (tx, rx) = mpsc::channel();

    // cpal streams are not Send on every platform, so one thread owns the
    // stream for the life of the app.
    thread::Builder::new()
        .name("earshot-mic".into())
        .spawn(move || match open(producer) {
            Ok((stream, device, rate)) => {
                let _ = tx.send(Ok((device, rate)));
                loop {
                    thread::park();
                    let _ = &stream;
                }
            }
            Err(err) => {
                let _ = tx.send(Err(err));
            }
        })
        .expect("failed to spawn the mic thread");

    commands.insert_resource(Capture {
        samples: Mutex::new(consumer),
        opened: Mutex::new(Some(rx)),
    });
}

pub(super) fn read(capture: Res<Capture>, mut mic: ResMut<Mic>) {
    mic.fresh = false;

    let mut opened = capture.opened.lock().unwrap();
    if let Some(rx) = opened.as_ref() {
        let result = match rx.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("the mic thread stopped".into())),
            Err(mpsc::TryRecvError::Empty) => None,
        };
        match result {
            Some(Ok((device, rate))) => {
                info!("Listening on {device} at {rate} Hz");
                mic.sample_rate = rate as f32;
                mic.status = MicStatus::Listening { device };
                *opened = None;
            }
            Some(Err(err)) => {
                error!("Couldn't open the mic: {err}");
                mic.status = MicStatus::Failed(err);
                *opened = None;
            }
            None => {}
        }
    }

    let mut samples = capture.samples.lock().unwrap();
    let ready = samples.slots();
    if let Ok(chunk) = samples.read_chunk(ready) {
        let (a, b) = chunk.as_slices();
        mic.push(a);
        mic.push(b);
        chunk.commit_all();
    }
}

fn open(producer: Producer<f32>) -> Result<(cpal::Stream, String, u32), String> {
    let host = host();
    let device = match std::env::var(DEVICE_ENV) {
        Ok(wanted) => find_device(&host, &wanted)?,
        Err(_) => host.default_input_device().ok_or("no input device found")?,
    };
    let name = device
        .description()
        .map(|d| d.name().to_owned())
        .unwrap_or_else(|_| "unknown device".into());
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let rate = config.sample_rate();

    let stream = match config.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config.into(), producer),
        SampleFormat::I16 => build::<i16>(&device, config.into(), producer),
        SampleFormat::I32 => build::<i32>(&device, config.into(), producer),
        SampleFormat::U16 => build::<u16>(&device, config.into(), producer),
        other => return Err(format!("unsupported sample format {other}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, name, rate))
}

#[cfg(target_os = "linux")]
fn host() -> cpal::Host {
    cpal::host_from_id(cpal::HostId::PipeWire).unwrap_or_else(|_| cpal::default_host())
}

#[cfg(not(target_os = "linux"))]
fn host() -> cpal::Host {
    cpal::default_host()
}

fn find_device(host: &cpal::Host, wanted: &str) -> Result<cpal::Device, String> {
    let named: Vec<_> = host
        .input_devices()
        .map_err(|e| e.to_string())?
        .filter_map(|d| {
            // Virtual sources can have no description, so fall back to the device id.
            let name = match d.description() {
                Ok(desc) if desc.name() != "unknown" => desc.name().to_owned(),
                _ => d.id().ok()?.to_string(),
            };
            Some((name, d))
        })
        .collect();
    let names: Vec<_> = named.iter().map(|(name, _)| name.as_str()).collect();
    let names = names.join(", ");
    named
        .into_iter()
        .find(|(name, _)| name.to_lowercase().contains(&wanted.to_lowercase()))
        .map(|(_, device)| device)
        .ok_or_else(|| format!("no input matching {DEVICE_ENV}={wanted}. Inputs: {names}"))
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut producer: Producer<f32>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &_| {
                for frame in data.chunks(channels) {
                    let mono =
                        frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / channels as f32;
                    // If Bevy falls a second behind, dropping audio is the right call.
                    let _ = producer.push(mono);
                }
            },
            |err| eprintln!("mic stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
}
