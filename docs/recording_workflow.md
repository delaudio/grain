# Grain Terminal Recording Workflow

This document details the deterministic terminal recording pipeline for **Grain**, driven by [`ttry`](https://github.com/delaudio/ttry) and external video capture.

---

## 1. Architecture & Pipeline

```text
ttry scenario / replay
        ↓
      Grain (TUI & p5.js Runtime)
        ↓
 Real Terminal (Ratty / Kitty / Ghostty)
        ↓
 External Window Capture (ffmpeg / screencapture / OBS)
        ↓
   Publishable MP4 Video
```

### Architectural Principles
1. **Separation of Concerns**: Grain core contains zero recording, ffmpeg, OBS, or ScreenCaptureKit code.
2. **Real Terminal Rendering**: The recorder captures the real terminal window, ensuring terminal graphics, fonts, ANSI colors, half-block shades, and glow effects are faithfully preserved.
3. **Deterministic Interaction**: `ttry` drives the keyboard input and state transitions deterministically without manual human intervention.

---

## 2. Recording a Demo

### Prerequisites
- **Rust Toolchain**: `cargo`
- **Audio Fixture**: `fixtures/demo.wav` (committed in repo)
- **Video Encoder**: `ffmpeg`
- **Scenario Runner**: `ttry` (or `./scripts/record_demo.sh`)

### Automated Run
To build Grain and replay the recording scenario:
```bash
./scripts/record_demo.sh
```

### Manual Capture Command (macOS Window Capture)
To capture a specific terminal window cleanly into an MP4:
```bash
# 1. Identify Terminal Window ID
# 2. Record using screencapture or ffmpeg avfoundation:
ffmpeg -f avfoundation -i "capture screen index:none" -r 60 -c:v libx264 -crf 18 -pix_fmt yuv420p recordings/grain_demo.mp4
```

---

## 3. Recommended Terminal Profiles

For social/vertical video formats (e.g. 9:16 reels, 4:5 posts):
- **Columns**: `70`
- **Rows**: `32`
- **Font**: JetBrains Mono or Fira Code Nerd Font
- **Color Theme**: Catppuccin Mocha or Tokyo Night

---

## 4. Replay Scenarios
- `scenarios/mvp_flow.yaml`: Fast E2E test verification.
- `scenarios/recording_demo.yaml`: Presentation-paced scenario with typing animations and viewing pauses.
