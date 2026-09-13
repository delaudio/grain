# Grain

> Terminal-first audio-reactive creative coding instrument.

Grain is a terminal application for generating, previewing, and revising audio-reactive creative visuals directly inside the terminal.

## Features

- **Terminal-First UI**: Built with Rust and [Ratatui](https://ratatui.rs).
- **Action/State Architecture**: Fully deterministic state transitions, easily drivable by [`ttry`](https://github.com/delaudio/ttry).
- **p5.js Sketch Runtime**: Deterministic frame-by-frame visual rendering driven by audio features.
- **Prompt-Driven Iteration**: Generate and revise visual sketches using natural language.
- **Version History & Rollback**: Instant switching between generated visual iterations.

## Keyboard Controls

| Key | Action |
|---|---|
| `o` | Open/load audio file (WAV / MP3) |
| `p` | Edit natural language generation prompt |
| `g` | Generate/regenerate sketch from active prompt |
| `Space` | Play / pause audio & visual preview playback |
| `v` | View sketch version history and rollback |
| `?` | Toggle help dialog |
| `q` / `Ctrl+C` | Quit Grain |

## Usage

Sketch evaluation uses embedded QuickJS; Node.js is not required. Sketches have
no filesystem, network, environment-variable or process APIs. Each evaluation
is limited to 250 ms of elapsed time (checked by the interpreter), 32 MiB of JS
heap, 256 KiB of JS stack, 256 KiB of source, and 4 MiB of serialized output.
Canvas dimensions must be 1..4096 on each axis and terminal output is limited
to 32,768 cells. Limit violations produce a runtime diagnostic, never a fake
successful preview. Building from source also requires a C toolchain for QuickJS.

The terminal currently implements a p5-like subset, not the complete browser
p5.js API. Evaluation is synchronous and starts with fresh sketch state for
each frame; persistent sketch lifecycle and higher-fidelity drawing are planned.
The embedded runtime exposes no host capabilities, but runs in the Grain process;
these limits are not OS process isolation or a hard real-time guarantee.

```bash
# Run Grain directly
cargo run

# Run with an audio file
cargo run -- path/to/track.wav
```
