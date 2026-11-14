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
| `crates/earshot-dsp` | Pitch detection (YIN), note maths, tuner logic. No Bevy, fast to test. |
| `crates/earshot` | The Bevy app: mic input and screens. |
| `crates/earshot/src/mic/native.rs` | Native capture with cpal (PipeWire on Linux), via a lock-free ring buffer. |
| `crates/earshot/src/mic/web.rs` | Browser capture with Web Audio (cpal has no mic input on the web). |

## Test

```sh
cargo test -p earshot-dsp
```

## Licences

Fira Sans (`crates/earshot/assets/fonts`) is under the SIL Open Font License, see `FiraSans-OFL.txt`.
