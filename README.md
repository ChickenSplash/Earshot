# Earshot

Play a real guitar into your mic and Earshot listens: which string, which note, how in tune.
Built in Rust with [Bevy](https://bevyengine.org). Native first, with a browser build too.

The long-term goal is a note highway you play along to, scored on what it hears.
The earlier JavaScript prototype is archived locally at `~/Projects/Archive/Earshot`.

## Run

```sh
cargo run --release
```

It uses the system default input. To pick another one, set `EARSHOT_INPUT` to part of its name:

```sh
EARSHOT_INPUT=maono cargo run --release
```

If the name doesn't match, the error lists every input it can see.

Two screens, switched with the bar at the top or Tab:

- **Tuner:** the nearest string and how many cents off it you are.
- **Monitor:** each note as you pluck it, with the ones before. Desktop only for now.

To start on a screen, pass its name: `cargo run --release -- monitor`.

### In the browser

Needs [Trunk](https://trunkrs.dev) (`pacman -S trunk`) and the wasm target (`rustup target add wasm32-unknown-unknown`).

```sh
cd crates/earshot
trunk serve --release
```

Open http://localhost:8080 and click once: browsers only allow the mic after a click.

## Layout

| Path | What |
|---|---|
| `crates/earshot-dsp` | Pitch detection (YIN), note detection (which note, when), note maths, tuner logic. No Bevy, fast to test. |
| `clips/` | Real recordings of an acoustic guitar through a USB mic, used by the tests. |
| `crates/earshot` | The Bevy app: mic input and screens. |
| `crates/earshot/src/mic/native.rs` | Native capture with cpal (PipeWire on Linux), via a lock-free ring buffer. |
| `crates/earshot/src/mic/web.rs` | Browser capture with Web Audio (cpal has no mic input on the web). |

## Test

```sh
cargo test -p earshot-dsp
```

`tests/clips.rs` checks note detection against the recordings in `clips/`, with pluck times read off the waveforms.

Two tools for looking at a recording:

```sh
cargo run -p earshot-dsp --example notes -- clips/*.wav        # the notes the detector finds
cargo run -p earshot-dsp --example track -- clips/open-e2.wav  # pitch and level over time
```

## Licences

Fira Sans (`crates/earshot/assets/fonts`) is under the SIL Open Font License, see `FiraSans-OFL.txt`.
