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

```bash
# Run Grain directly
cargo run

# Run with an audio file
cargo run -- path/to/track.wav
```
