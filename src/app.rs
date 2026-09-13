use crate::action::Action;
use crate::state::{AudioStatus, GenerationStatus, GrainState, InputMode, PreviewStatus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use crate::generator::GenerationService;
use crate::history::HistoryManager;
use crate::history::record::VersionInputs;
use crate::runtime::engine::{CONTRACT_VERSION, EngineId};
use crate::runtime::parameters::Parameters;

type PreviewSignature = (String, u64, u32, u32, u16, u16, EngineId, Parameters);

#[derive(Clone)]
struct SketchDraft {
    params: Parameters,
    source: String,
    seed: u64,
    prompt: String,
}

pub struct App {
    image_target: Option<(ratatui::layout::Rect, ratatui::layout::Rect)>,
    pending_image_packet: Option<Vec<u8>>,
    image_active: bool,
    image_pixel_budget: usize,
    image_interval: std::time::Duration,
    next_image_request: Instant,
    image_preparation_time: std::time::Duration,
    sketch_drafts: [Option<SketchDraft>; 2],
    pub state: GrainState,
    pub history_manager: HistoryManager,
    pub audio_player: crate::audio::AudioPlayer,
    pub last_watched_mtime: Option<SystemTime>,
    preview_worker: Option<crate::preview::worker::PreviewWorker>,
    preview_signature: Option<PreviewSignature>,
    preview_revision: u64,
    preview_lifecycle: u64,
    playback_loop: usize,
    audio_generation: u64,
    requested_frame: Option<usize>,
    requested_audio: Option<crate::audio::AudioFeatures>,
    playback_clock: crate::preview::clock::PlaybackClock,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self::with_history_manager(HistoryManager::default())
    }

    pub fn with_history_manager(history_manager: HistoryManager) -> Self {
        let mut state = GrainState::default();
        let mut last_watched_mtime = None;

        if let Ok(history) = history_manager.load_history()
            && !history.versions.is_empty()
        {
            state.prompt.total_versions = history.versions.len();
            state.prompt.current_version = history.active_version;
            state.versions.selected_index = history.active_version.saturating_sub(1);
            state.versions.history = history.clone();

            if let Some(active_meta) = history
                .versions
                .iter()
                .find(|v| v.version == history.active_version)
            {
                if active_meta.contract_version == CONTRACT_VERSION
                    && active_meta.runtime_contract == active_meta.engine.contract_name()
                {
                    state.preview.seed = active_meta.seed;
                    state.preview.params = active_meta.params.clone();
                    state.preview.engine = active_meta.engine;
                    state.selected_sketch_engine = active_meta.engine;
                    state.prompt.active_prompt = active_meta.prompt.clone();
                    let restored = history_manager
                        .load_sketch_content(&active_meta.sketch_file)
                        .map_err(|error| error.to_string())
                        .and_then(|code| {
                            GenerationService::validate_with_parameters(
                                active_meta.engine,
                                &code,
                                active_meta.seed,
                                &active_meta.params,
                            )?;
                            Ok(code)
                        });
                    match restored {
                        Ok(code) => {
                            state.preview.sketch_name = format!("sketch_v{}", active_meta.version);
                            state.preview.sketch_source = code;
                        }
                        Err(error) => {
                            // Keep the matching engine so the original file can
                            // still be repaired via editor/hot reload. Never
                            // overwrite the invalid source or its history entry.
                            state.preview.sketch_source = match active_meta.engine {
                                EngineId::P5 => crate::runtime::template::DEFAULT_SKETCH_TEMPLATE,
                                EngineId::Ascii => {
                                    crate::runtime::template::DEFAULT_ASCII_SKETCH_TEMPLATE
                                }
                            }
                            .into();
                            state.preview.sketch_name =
                                format!("recovery_{}", active_meta.engine.label());
                            state.prompt.current_version = 0;
                            state.status_message = Some(format!(
                                "Cannot restore sketch: {error}. Using recovery template; original file unchanged."
                            ));
                        }
                    }
                    state.preview.status = PreviewStatus::Ready;
                } else {
                    state.prompt.current_version = 0;
                    state.status_message =
                        Some("Cannot restore sketch: unsupported engine contract".into());
                }
                let p = history_manager.get_sketch_path(&active_meta.sketch_file);
                if let Ok(meta) = std::fs::metadata(&p) {
                    last_watched_mtime = meta.modified().ok();
                }
            }
        }

        let preview_worker = match crate::preview::worker::PreviewWorker::new() {
            Ok(worker) => Some(worker),
            Err(error) => {
                state.status_message = Some(format!("Cannot start preview worker: {error}"));
                None
            }
        };
        Self {
            sketch_drafts: [None, None],
            image_target: None,
            pending_image_packet: None,
            image_active: false,
            image_pixel_budget: crate::preview::iterm2::MAX_IMAGE_PIXELS,
            image_interval: std::time::Duration::from_secs_f64(1.0 / 30.0),
            next_image_request: Instant::now(),
            image_preparation_time: std::time::Duration::ZERO,
            state,
            history_manager,
            audio_player: crate::audio::AudioPlayer::new(),
            last_watched_mtime,
            preview_worker,
            preview_signature: None,
            preview_revision: 0,
            preview_lifecycle: 0,
            playback_loop: 0,
            audio_generation: 0,
            requested_frame: None,
            requested_audio: None,
            playback_clock: crate::preview::clock::PlaybackClock::default(),
        }
    }

    #[allow(dead_code)]
    pub fn with_audio_file(mut self, path: PathBuf) -> Self {
        self.load_audio(path);
        self
    }

    pub fn load_audio(&mut self, path: PathBuf) {
        let file_name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        self.playback_clock.reset(Instant::now());
        self.preview_lifecycle = self.preview_lifecycle.wrapping_add(1);
        self.preview_revision = self.preview_revision.wrapping_add(1);
        self.requested_frame = None;
        self.playback_loop = 0;
        self.state.preview.current_frame = 0;
        self.state.audio.path = Some(path.clone());
        self.state.audio.status = AudioStatus::Loading;
        self.state.status_message = Some(format!("Analyzing audio: {}", file_name));

        let _ = self.audio_player.load(&path);

        match crate::audio::load_or_analyze(&path, self.state.preview.fps) {
            Ok(analysis) => {
                self.state.audio.duration_ms = analysis.duration_ms;
                self.state.audio.sample_rate = analysis.sample_rate;
                self.state.audio.channels = analysis.channels;
                self.state.preview.total_frames = analysis.total_frames();
                self.state.audio.status = AudioStatus::Ready;
                self.state.status_message = Some(format!(
                    "Ready: {} ({:.1}s)",
                    file_name,
                    analysis.duration_ms as f64 / 1000.0
                ));
                self.state.audio.analysis = Some(analysis);
            }
            Err(err) => {
                self.state.audio.status = AudioStatus::Error(err.to_string());
                self.state.status_message = Some(format!("Audio error: {}", err));
            }
        }
    }

    pub fn update(&mut self, action: Action) -> Option<Action> {
        match action {
            Action::Quit => {
                self.state.should_quit = true;
            }
            Action::Tick => {
                self.advance_playback(Instant::now());

                // Compute real-time DSP-processed audio features for visual engine and UI
                if let Some(ref analysis) = self.state.audio.analysis {
                    let raw = analysis.get_features_at_frame(self.state.preview.current_frame);
                    let peak = analysis.peak_features();
                    let prev = self.state.live_audio_features;
                    self.state.live_audio_features = crate::audio::process_features(
                        raw,
                        &self.state.dsp,
                        Some(peak),
                        Some(prev),
                    );
                }

                // Check for external file modifications in .grain/sketches/ and hot reload live
                self.check_and_reload_sketch_from_disk();
            }
            Action::Resize(w, h) => {
                self.state.terminal_size = (w, h);
            }
            Action::ToggleTuningModal => {
                self.state.mode = match self.state.mode {
                    InputMode::Tuning => InputMode::Normal,
                    _ => InputMode::Tuning,
                };
            }
            Action::TuningNextParam => {
                self.state.tuning_selected_param = (self.state.tuning_selected_param + 1) % 7;
            }
            Action::TuningPrevParam => {
                self.state.tuning_selected_param = if self.state.tuning_selected_param == 0 {
                    6
                } else {
                    self.state.tuning_selected_param - 1
                };
            }
            Action::TuningIncreaseParam => match self.state.tuning_selected_param {
                0 => self.state.dsp.master_gain = (self.state.dsp.master_gain + 0.1).min(4.0),
                1 => self.state.dsp.low_gain = (self.state.dsp.low_gain + 0.1).min(4.0),
                2 => self.state.dsp.mid_gain = (self.state.dsp.mid_gain + 0.1).min(4.0),
                3 => self.state.dsp.high_gain = (self.state.dsp.high_gain + 0.1).min(4.0),
                4 => self.state.dsp.threshold = (self.state.dsp.threshold + 0.01).min(0.5),
                5 => self.state.dsp.attack_decay = (self.state.dsp.attack_decay + 0.05).min(0.95),
                6 => self.state.dsp.auto_gain = !self.state.dsp.auto_gain,
                _ => {}
            },
            Action::TuningDecreaseParam => match self.state.tuning_selected_param {
                0 => self.state.dsp.master_gain = (self.state.dsp.master_gain - 0.1).max(0.1),
                1 => self.state.dsp.low_gain = (self.state.dsp.low_gain - 0.1).max(0.1),
                2 => self.state.dsp.mid_gain = (self.state.dsp.mid_gain - 0.1).max(0.1),
                3 => self.state.dsp.high_gain = (self.state.dsp.high_gain - 0.1).max(0.1),
                4 => self.state.dsp.threshold = (self.state.dsp.threshold - 0.01).max(0.0),
                5 => self.state.dsp.attack_decay = (self.state.dsp.attack_decay - 0.05).max(0.0),
                6 => self.state.dsp.auto_gain = !self.state.dsp.auto_gain,
                _ => {}
            },
            Action::TuningResetDefaults => {
                self.state.dsp = crate::audio::DspSettings::default();
                self.state.status_message = Some("Reset DSP tuning to default preset".to_string());
            }
            Action::ToggleSelectModel => {
                self.state.mode = match self.state.mode {
                    InputMode::SelectModel => InputMode::Normal,
                    _ => {
                        self.state.engine.selected_index = self.state.engine.active_index;
                        InputMode::SelectModel
                    }
                };
            }
            Action::ToggleSketchEngine => {
                self.state.mode = if self.state.mode == InputMode::SelectSketchEngine {
                    InputMode::Normal
                } else {
                    self.state.selected_sketch_engine = self.state.preview.engine;
                    InputMode::SelectSketchEngine
                };
            }
            Action::EnterParameters => {
                self.state.parameter_input_buffer.clear();
                self.state.mode = InputMode::EditingParameter;
            }
            Action::ExitParameters => self.state.mode = InputMode::Normal,
            Action::ParameterInputChar(ch) => {
                if !ch.is_control()
                    && self.state.parameter_input_buffer.len() + ch.len_utf8() <= 256
                {
                    self.state.parameter_input_buffer.push(ch);
                }
            }
            Action::ParameterBackspace => {
                self.state.parameter_input_buffer.pop();
            }
            Action::CommitParameter => match self.commit_parameter() {
                Ok(()) => self.state.mode = InputMode::Normal,
                Err(error) => {
                    self.state.status_message = Some(format!("Parameter not changed: {error}"))
                }
            },
            Action::RestartSketch => {
                self.preview_lifecycle = self.preview_lifecycle.wrapping_add(1);
                self.preview_revision = self.preview_revision.wrapping_add(1);
                self.requested_frame = None;
                self.state.status_message =
                    Some("Reset sketch state at the current playback position".into());
            }
            Action::SelectSketchEngine(engine) => self.state.selected_sketch_engine = engine,
            Action::ActivateSketchEngine => {
                if let Err(error) = self.activate_sketch_engine(self.state.selected_sketch_engine) {
                    self.state.status_message = Some(format!("Engine switch cancelled: {error}"));
                }
            }
            Action::SelectPreviousEngine => {
                if self.state.engine.selected_index > 0 {
                    self.state.engine.selected_index -= 1;
                }
            }
            Action::SelectNextEngine => {
                if self.state.engine.selected_index + 1 < self.state.engine.options.len() {
                    self.state.engine.selected_index += 1;
                }
            }
            Action::ActivateSelectedEngine => {
                self.state.engine.active_index = self.state.engine.selected_index;
                self.state.mode = InputMode::Normal;
                let label = self.state.engine.active_label().to_string();
                self.state.status_message = Some(format!("Switched AI Engine to: {}", label));
            }
            Action::ToggleHelp => {
                self.state.mode = match self.state.mode {
                    InputMode::Help => InputMode::Normal,
                    _ => InputMode::Help,
                };
            }
            Action::ToggleVersions => {
                self.state.mode = match self.state.mode {
                    InputMode::Versions => InputMode::Normal,
                    _ => {
                        if !self.state.versions.history.versions.is_empty() {
                            self.state.versions.selected_index = self
                                .state
                                .versions
                                .history
                                .active_version
                                .saturating_sub(1)
                                .min(self.state.versions.history.versions.len() - 1);
                        }
                        InputMode::Versions
                    }
                };
            }
            Action::SelectPreviousVersion => {
                if self.state.versions.selected_index > 0 {
                    self.state.versions.selected_index -= 1;
                }
            }
            Action::SelectNextVersion => {
                if !self.state.versions.history.versions.is_empty()
                    && self.state.versions.selected_index + 1
                        < self.state.versions.history.versions.len()
                {
                    self.state.versions.selected_index += 1;
                }
            }
            Action::RollbackToSelectedVersion => {
                if let Some(v_meta) = self
                    .state
                    .versions
                    .history
                    .versions
                    .get(self.state.versions.selected_index)
                {
                    return Some(Action::RollbackToVersion(v_meta.version));
                }
            }
            Action::RollbackToVersion(v) => {
                if let Some(v_meta) = self
                    .state
                    .versions
                    .history
                    .versions
                    .iter()
                    .find(|m| m.version == v)
                    .cloned()
                    && let Ok(content) = self
                        .history_manager
                        .load_sketch_content(&v_meta.sketch_file)
                {
                    if v_meta.contract_version != CONTRACT_VERSION
                        || v_meta.runtime_contract != v_meta.engine.contract_name()
                    {
                        self.state.status_message =
                            Some("Rollback cancelled: unsupported engine contract".into());
                        return None;
                    }
                    if let Err(error) = GenerationService::validate_with_parameters(
                        v_meta.engine,
                        &content,
                        v_meta.seed,
                        &v_meta.params,
                    ) {
                        self.state.status_message = Some(format!("Rollback cancelled: {error}"));
                        return None;
                    }
                    let mut history = self.state.versions.history.clone();
                    history.active_version = v_meta.version;
                    if let Err(error) = self.history_manager.save_history(&history) {
                        self.state.status_message = Some(format!("Rollback not saved: {error}"));
                        return None;
                    }
                    self.adopt_version(&v_meta, content);
                    self.state.status_message =
                        Some(format!("Rolled back to visual sketch v{}", v_meta.version));
                }
            }
            Action::TogglePlayback => {
                self.playback_clock.update(
                    Instant::now(),
                    self.state.preview.is_playing,
                    self.audio_player.position(),
                );
                self.state.preview.is_playing = !self.state.preview.is_playing;
                if self.state.preview.is_playing {
                    self.audio_player.play();
                } else {
                    self.audio_player.pause();
                }
                self.playback_clock.update(
                    Instant::now(),
                    self.state.preview.is_playing,
                    self.audio_player.position(),
                );
                self.state.status_message = Some(if self.state.preview.is_playing {
                    "Playback: Playing".to_string()
                } else {
                    "Playback: Paused".to_string()
                });
            }
            Action::EnterPromptEdit => {
                self.state.mode = InputMode::EditingPrompt;
                self.state.prompt.input_buffer = self.state.prompt.active_prompt.clone();
                self.state.prompt.cursor_position = self.state.prompt.input_buffer.chars().count();
            }
            Action::ExitPromptEdit => {
                self.state.mode = InputMode::Normal;
                self.state.prompt.input_buffer.clear();
            }
            Action::PromptInputChar(c) => {
                let char_idx = self.state.prompt.cursor_position;
                let byte_idx = self
                    .state
                    .prompt
                    .input_buffer
                    .char_indices()
                    .nth(char_idx)
                    .map(|(i, _)| i)
                    .unwrap_or(self.state.prompt.input_buffer.len());
                self.state.prompt.input_buffer.insert(byte_idx, c);
                self.state.prompt.cursor_position += 1;
            }
            Action::PromptBackspace => {
                if self.state.prompt.cursor_position > 0 {
                    let char_idx = self.state.prompt.cursor_position - 1;
                    let byte_idx = self
                        .state
                        .prompt
                        .input_buffer
                        .char_indices()
                        .nth(char_idx)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.state.prompt.input_buffer.remove(byte_idx);
                    self.state.prompt.cursor_position -= 1;
                }
            }
            Action::PromptDelete => {
                let char_idx = self.state.prompt.cursor_position;
                if char_idx < self.state.prompt.input_buffer.chars().count() {
                    let byte_idx = self
                        .state
                        .prompt
                        .input_buffer
                        .char_indices()
                        .nth(char_idx)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.state.prompt.input_buffer.remove(byte_idx);
                }
            }
            Action::PromptCursorLeft => {
                if self.state.prompt.cursor_position > 0 {
                    self.state.prompt.cursor_position -= 1;
                }
            }
            Action::PromptCursorRight => {
                if self.state.prompt.cursor_position
                    < self.state.prompt.input_buffer.chars().count()
                {
                    self.state.prompt.cursor_position += 1;
                }
            }
            Action::CommitPrompt => {
                let trimmed = self.state.prompt.input_buffer.trim().to_string();
                if !trimmed.is_empty() {
                    self.state.prompt.active_prompt = trimmed;
                }
                self.state.mode = InputMode::Normal;
                return Some(Action::TriggerGenerate);
            }
            Action::TriggerGenerate => {
                self.state.prompt.generation_status = GenerationStatus::Generating;
                self.state.status_message = Some(format!(
                    "Generating audio-reactive visual via {}...",
                    self.state.engine.active_label()
                ));

                let prompt = self.state.prompt.active_prompt.clone();
                let seed = self.state.preview.seed.wrapping_add(1);
                let engine = self.state.preview.engine;
                let params = self.state.preview.params.clone();
                let is_revision = self.state.prompt.total_versions > 0;
                let current_sketch = self.state.preview.sketch_source.clone();

                let service = self.state.engine.create_service_for_active();
                let result = if is_revision {
                    service.revise_with_parameters(engine, &prompt, &current_sketch, seed, &params)
                } else {
                    service.generate_with_parameters(engine, &prompt, seed, &params)
                };

                return Some(Action::GenerationCompleted {
                    params,
                    result,
                    prompt,
                    engine,
                    seed,
                });
            }
            Action::GenerationCompleted {
                params,
                result,
                prompt,
                engine,
                seed,
            } => {
                if engine != self.state.preview.engine || params != self.state.preview.params {
                    self.state.status_message =
                        Some("Discarded generation for an inactive sketch engine".into());
                    return None;
                }
                match result {
                    Ok(new_code) => {
                        let audio_hash = self.state.audio.path.as_ref().and_then(|p| {
                            crate::audio::get_cache_path(p, self.state.preview.fps)
                                .ok()
                                .and_then(|cp| {
                                    cp.file_stem().map(|s| s.to_string_lossy().to_string())
                                })
                        });

                        let saved = self.history_manager.record_version(
                            &prompt,
                            &new_code,
                            "Grain Generator",
                            audio_hash.as_deref(),
                            &VersionInputs {
                                engine,
                                seed,
                                params,
                            },
                        );
                        let meta = match saved {
                            Ok(meta) => meta,
                            Err(error) => {
                                self.state.prompt.generation_status =
                                    GenerationStatus::Failed(error.to_string());
                                self.state.status_message = Some(format!(
                                    "Generation not saved; previous sketch retained: {error}"
                                ));
                                return None;
                            }
                        };
                        self.adopt_version(&meta, new_code);
                        self.state.preview.is_playing = true;
                        self.audio_player.play();
                        self.state.prompt.generation_status = GenerationStatus::Ready;
                        self.state.preview.status = PreviewStatus::Ready;
                        self.state.status_message = Some(format!(
                            "Active visual: sketch_v{} (Prompt: \"{}\")",
                            self.state.prompt.current_version, self.state.prompt.active_prompt
                        ));
                    }
                    Err(err) => {
                        self.state.prompt.generation_status = GenerationStatus::Failed(err.clone());
                        self.state.status_message = Some(format!("Generation error: {}", err));
                    }
                }
            }
            Action::EnterOpenAudio => {
                self.state.mode = InputMode::OpeningAudio;
                self.state.audio_input_buffer.clear();
            }
            Action::ExitOpenAudio => {
                self.state.mode = InputMode::Normal;
                self.state.audio_input_buffer.clear();
            }
            Action::AudioInputChar(c) => {
                self.state.audio_input_buffer.push(c);
            }
            Action::AudioBackspace => {
                self.state.audio_input_buffer.pop();
            }
            Action::CommitOpenAudio => {
                let path_str = self.state.audio_input_buffer.trim().to_string();
                self.state.mode = InputMode::Normal;
                if !path_str.is_empty() {
                    return Some(Action::LoadAudio(PathBuf::from(path_str)));
                }
            }
            Action::LoadAudio(path) => {
                self.load_audio(path);
            }
            Action::OpenInEditor => {
                // Handled in main loop (suspends TUI, opens $EDITOR)
            }
            Action::OpenInBrowser => {
                if self.state.preview.engine == EngineId::Ascii {
                    self.state.status_message = Some("Native ASCII is rendered in the terminal; the browser viewer supports p5 only".into());
                    return None;
                }
                let url = "http://127.0.0.1:3333";
                #[cfg(target_os = "macos")]
                let _ = std::process::Command::new("open").arg(url).spawn();
                #[cfg(target_os = "linux")]
                let _ = std::process::Command::new("xdg-open").arg(url).spawn();
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("cmd")
                    .args(["/C", "start", url])
                    .spawn();
                self.state.status_message = Some(
                    "🌐 Opened native p5.js canvas in browser at http://localhost:3333".to_string(),
                );
            }
            Action::SetStatusMessage(msg) => {
                self.state.status_message = Some(msg);
            }
        }
        self.service_preview();
        None
    }

    /// Input-thread work is limited to mailbox swaps and adopting ready output.
    pub fn service_preview(&mut self) {
        let area = crate::ui::preview_content_rect(ratatui::layout::Rect::new(
            0,
            0,
            self.state.terminal_size.0,
            self.state.terminal_size.1,
        ));
        let preview = &self.state.preview;
        let signature = (
            preview.sketch_source.clone(),
            preview.seed,
            preview.width,
            preview.height,
            area.width,
            area.height,
            preview.engine,
            preview.params.clone(),
        );
        let changed = self.preview_signature.as_ref() != Some(&signature);
        let lifecycle_changed = self.preview_signature.as_ref().is_none_or(|previous| {
            previous.0 != signature.0
                || previous.1 != signature.1
                || previous.2 != signature.2
                || previous.3 != signature.3
                || previous.6 != signature.6
        });
        let backwards = self
            .requested_frame
            .is_some_and(|frame| preview.current_frame < frame);
        if lifecycle_changed || backwards {
            self.preview_lifecycle = self.preview_lifecycle.wrapping_add(1);
        }
        if changed || backwards {
            self.pending_image_packet = None;
            self.preview_revision = self.preview_revision.wrapping_add(1);
            self.preview_signature = Some(signature);
            self.requested_frame = None;
        }
        let Some(worker) = &self.preview_worker else {
            return;
        };
        if let Some(mut completed) = worker.take_engine_completed().filter(|r| {
            r.revision == self.preview_revision && r.engine == self.state.preview.engine
        }) {
            if let Some(Ok(packet)) = completed.image_packet.take() {
                self.image_preparation_time = completed.preparation_time;
                self.pending_image_packet = Some(packet);
                self.image_active = true;
                self.state.preview.active_frame_result = None;
                self.state.preview.runtime_error = None;
                self.state.preview.status = PreviewStatus::Ready;
            } else {
                match completed.into_terminal().result {
                    Ok(result) => {
                        self.image_active = false;
                        self.state.preview.active_frame_result = Some(result);
                        self.state.preview.runtime_error = None;
                        self.state.preview.status = PreviewStatus::Ready;
                    }
                    Err(error) => {
                        self.state.status_message =
                            Some(format!("Sketch error (last valid frame retained): {error}"));
                        self.state.preview.status = PreviewStatus::Error(error.to_string());
                        self.state.preview.runtime_error = Some(error);
                    }
                }
            }
            if area.width == 0 {
                return;
            }
        }
        if area.width == 0
            || area.height == 0
            || (self.requested_frame == Some(self.state.preview.current_frame)
                && self.requested_audio == Some(self.state.live_audio_features))
        {
            return;
        }
        self.requested_frame = Some(self.state.preview.current_frame);
        if self.image_target.is_some() {
            if Instant::now() < self.next_image_request {
                self.requested_frame = None;
                return;
            }
            self.next_image_request = Instant::now() + self.image_interval;
        }
        self.requested_audio = Some(self.state.live_audio_features);
        let request = crate::preview::worker::RenderRequest {
            revision: self.preview_revision,
            lifecycle: self.preview_lifecycle,
            source: std::sync::Arc::from(self.state.preview.sketch_source.as_str()),
            context: crate::runtime::GrainContext {
                params: self.state.preview.params.clone(),
                width: self.state.preview.width,
                height: self.state.preview.height,
                seed: self.state.preview.seed,
                frame: self.state.preview.current_frame,
                time: self.state.preview.current_frame as f64
                    / self.state.preview.fps.max(1) as f64,
                audio: self.state.live_audio_features,
            },
            cols: area.width,
            rows: area.height,
        };
        if let Some((area, screen)) = self.image_target {
            worker.submit_iterm2_with_budget(
                self.state.preview.engine,
                request,
                area,
                screen,
                self.image_pixel_budget,
            );
        } else {
            worker.submit_for_engine(self.state.preview.engine, request);
        }
    }

    pub fn set_image_target(
        &mut self,
        target: Option<(ratatui::layout::Rect, ratatui::layout::Rect)>,
    ) {
        if target != self.image_target {
            self.image_target = target;
            self.next_image_request = Instant::now();
            self.preview_revision = self.preview_revision.wrapping_add(1);
            self.requested_frame = None;
            self.pending_image_packet = None;
            self.image_active = false;
        }
    }

    pub fn take_image_packet(&mut self) -> Option<Vec<u8>> {
        self.pending_image_packet.take()
    }

    pub fn image_active(&self) -> bool {
        self.image_active
    }

    pub fn set_image_pacing(&mut self, pixels: usize, interval: std::time::Duration) {
        let pixels = pixels.clamp(4096, crate::preview::iterm2::MAX_IMAGE_PIXELS);
        if self.image_pixel_budget != pixels {
            self.image_pixel_budget = pixels;
            self.preview_revision = self.preview_revision.wrapping_add(1);
            self.requested_frame = None;
            self.pending_image_packet = None;
        }
        self.image_interval = interval.clamp(
            std::time::Duration::from_secs_f64(1.0 / 30.0),
            std::time::Duration::from_secs(1),
        );
    }

    pub fn image_preparation_time(&self) -> std::time::Duration {
        self.image_preparation_time
    }

    fn advance_playback(&mut self, now: Instant) {
        if self.state.preview.is_playing && self.state.audio.analysis.is_some() {
            self.audio_player.play();
        }
        let position = self.playback_clock.update(
            now,
            self.state.preview.is_playing,
            self.audio_player.position(),
        );
        let absolute_frame =
            crate::preview::clock::PlaybackClock::frame(position, self.state.preview.fps, 0);
        let loop_index = absolute_frame
            .checked_div(self.state.preview.total_frames)
            .unwrap_or(0);
        let audio_generation = self.audio_player.generation();
        if loop_index != self.playback_loop || audio_generation != self.audio_generation {
            // This epoch survives replacement of the request at a loop boundary.
            self.preview_lifecycle = self.preview_lifecycle.wrapping_add(1);
            self.preview_revision = self.preview_revision.wrapping_add(1);
            self.requested_frame = None;
        }
        self.playback_loop = loop_index;
        self.audio_generation = audio_generation;
        self.state.preview.current_frame = crate::preview::clock::PlaybackClock::frame(
            position,
            self.state.preview.fps,
            self.state.preview.total_frames,
        );
    }

    pub fn check_and_reload_sketch_from_disk(&mut self) {
        if let Ok(Some(path)) = self.history_manager.get_active_sketch_path()
            && let Ok(meta) = std::fs::metadata(&path)
            && let Ok(mtime) = meta.modified()
        {
            if let Some(last_mtime) = self.last_watched_mtime {
                if mtime > last_mtime {
                    if let Ok(content) = std::fs::read_to_string(&path)
                        && content != self.state.preview.sketch_source
                    {
                        self.replace_edited_source(content);
                    }
                    self.last_watched_mtime = Some(mtime);
                }
            } else {
                self.last_watched_mtime = Some(mtime);
            }
        }
    }

    fn commit_parameter(&mut self) -> Result<(), String> {
        let command = self.state.parameter_input_buffer.trim().to_string();
        let mut params = self.state.preview.params.clone();
        if let Some((name, value)) = command.split_once('=') {
            let value = value
                .trim()
                .parse::<f64>()
                .map_err(|_| "Use a numeric value, for example speed=1.5")?;
            params.set(name.trim(), value).map_err(|e| e.to_string())?;
        } else if let Some(name) = command.strip_prefix('-') {
            params.remove(name.trim());
        } else {
            return Err("Use name=value to set a parameter or -name to remove it".into());
        }
        if params == self.state.preview.params {
            return Ok(());
        }
        GenerationService::validate_with_parameters(
            self.state.preview.engine,
            &self.state.preview.sketch_source,
            self.state.preview.seed,
            &params,
        )?;
        let inputs = VersionInputs {
            engine: self.state.preview.engine,
            seed: self.state.preview.seed,
            params: params.clone(),
        };
        let audio_hash = self
            .state
            .versions
            .history
            .versions
            .iter()
            .find(|v| v.version == self.state.prompt.current_version)
            .and_then(|v| v.audio_source_hash.clone());
        let meta = self
            .history_manager
            .record_version(
                &self.state.prompt.active_prompt,
                &self.state.preview.sketch_source,
                "Parameter change",
                audio_hash.as_deref(),
                &inputs,
            )
            .map_err(|e| e.to_string())?;
        self.state.preview.params = params.clone();
        self.sketch_drafts[inputs.engine.index()] = Some(SketchDraft {
            params,
            source: self.state.preview.sketch_source.clone(),
            seed: inputs.seed,
            prompt: self.state.prompt.active_prompt.clone(),
        });
        self.state.prompt.current_version = meta.version;
        self.state.preview.sketch_name = format!("sketch_v{}", meta.version);
        if let Ok(history) = self.history_manager.load_history() {
            self.state.prompt.total_versions = history.versions.len();
            self.state.versions.selected_index = history.versions.len().saturating_sub(1);
            self.state.versions.history = history;
        }
        self.requested_frame = None;
        self.state.status_message = Some(format!("Saved parameters in sketch_v{}", meta.version));
        Ok(())
    }

    fn adopt_version(&mut self, meta: &crate::history::record::VersionMetadata, source: String) {
        self.sketch_drafts[meta.engine.index()] = Some(SketchDraft {
            source: source.clone(),
            seed: meta.seed,
            params: meta.params.clone(),
            prompt: meta.prompt.clone(),
        });
        self.state.preview.engine = meta.engine;
        self.state.selected_sketch_engine = meta.engine;
        self.state.preview.seed = meta.seed;
        self.state.preview.params = meta.params.clone();
        self.state.preview.sketch_source = source;
        self.state.preview.sketch_name = format!("sketch_v{}", meta.version);
        self.state.preview.status = PreviewStatus::Ready;
        self.state.preview.runtime_error = None;
        self.state.preview.active_frame_result = None;
        self.state.prompt.active_prompt = meta.prompt.clone();
        self.state.prompt.current_version = meta.version;
        if let Ok(history) = self.history_manager.load_history() {
            self.state.prompt.total_versions = history.versions.len();
            self.state.versions.selected_index = history
                .versions
                .iter()
                .position(|v| v.version == meta.version)
                .unwrap_or(0);
            self.state.versions.history = history;
        }
        self.state.mode = InputMode::Normal;
        self.preview_signature = None;
        self.requested_frame = None;
        self.last_watched_mtime =
            std::fs::metadata(self.history_manager.get_sketch_path(&meta.sketch_file))
                .ok()
                .and_then(|meta| meta.modified().ok());
    }

    fn activate_sketch_engine(&mut self, engine: EngineId) -> Result<(), String> {
        if engine == self.state.preview.engine {
            self.state.mode = InputMode::Normal;
            return Ok(());
        }
        let candidate = if let Some(draft) = &self.sketch_drafts[engine.index()] {
            draft.clone()
        } else if let Some(meta) = self
            .state
            .versions
            .history
            .versions
            .iter()
            .rev()
            .find(|meta| {
                meta.engine == engine
                    && meta.contract_version == CONTRACT_VERSION
                    && meta.runtime_contract == engine.contract_name()
            })
        {
            SketchDraft {
                source: self
                    .history_manager
                    .load_sketch_content(&meta.sketch_file)
                    .map_err(|e| e.to_string())?,
                seed: meta.seed,
                params: meta.params.clone(),
                prompt: meta.prompt.clone(),
            }
        } else {
            SketchDraft {
                source: match engine {
                    EngineId::P5 => crate::runtime::template::DEFAULT_SKETCH_TEMPLATE,
                    EngineId::Ascii => crate::runtime::template::DEFAULT_ASCII_SKETCH_TEMPLATE,
                }
                .into(),
                seed: self.state.preview.seed,
                params: self.state.preview.params.clone(),
                prompt: self.state.prompt.active_prompt.clone(),
            }
        };
        GenerationService::validate_with_parameters(
            engine,
            &candidate.source,
            candidate.seed,
            &candidate.params,
        )?;
        let previous_engine = self.state.preview.engine;
        let previous = SketchDraft {
            source: self.state.preview.sketch_source.clone(),
            seed: self.state.preview.seed,
            params: self.state.preview.params.clone(),
            prompt: self.state.prompt.active_prompt.clone(),
        };
        // Preserve even an unversioned or externally edited source before switching.
        let saved = self
            .history_manager
            .record_version(
                &previous.prompt,
                &previous.source,
                "Before engine switch",
                None,
                &VersionInputs {
                    engine: previous_engine,
                    seed: previous.seed,
                    params: previous.params.clone(),
                },
            )
            .map_err(|e| e.to_string())?;
        self.adopt_version(&saved, previous.source.clone());
        self.sketch_drafts[previous_engine.index()] = Some(previous);
        let saved = self
            .history_manager
            .record_version(
                &candidate.prompt,
                &candidate.source,
                "Engine switch",
                None,
                &VersionInputs {
                    engine,
                    seed: candidate.seed,
                    params: candidate.params.clone(),
                },
            )
            .map_err(|e| e.to_string())?;
        self.adopt_version(&saved, candidate.source);
        self.state.status_message = Some(format!(
            "Sketch engine: {}; previous source preserved in history",
            engine.label()
        ));
        Ok(())
    }

    pub fn replace_edited_source(&mut self, source: String) -> bool {
        match GenerationService::validate_with_parameters(
            self.state.preview.engine,
            &source,
            self.state.preview.seed,
            &self.state.preview.params,
        ) {
            Ok(()) => {
                self.sketch_drafts[self.state.preview.engine.index()] = Some(SketchDraft {
                    source: source.clone(),
                    seed: self.state.preview.seed,
                    params: self.state.preview.params.clone(),
                    prompt: self.state.prompt.active_prompt.clone(),
                });
                self.state.preview.sketch_source = source;
                self.state.status_message = Some("Reloaded valid sketch changes from disk".into());
                true
            }
            Err(error) => {
                self.state.status_message = Some(format!(
                    "Edited sketch rejected; previous source retained: {error}"
                ));
                false
            }
        }
    }

    pub fn handle_key_event(&self, key: KeyEvent) -> Option<Action> {
        // Global quit shortcut
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Some(Action::Quit);
        }

        match self.state.mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('q') => Some(Action::Quit),
                KeyCode::Char('?') => Some(Action::ToggleHelp),
                KeyCode::Char('p') => Some(Action::EnterPromptEdit),
                KeyCode::Char('g') => Some(Action::TriggerGenerate),
                KeyCode::Char(' ') => Some(Action::TogglePlayback),
                KeyCode::Char('t') | KeyCode::Char('a') => Some(Action::ToggleTuningModal),
                KeyCode::Char('v') => Some(Action::ToggleVersions),
                KeyCode::Char('m') => Some(Action::ToggleSelectModel),
                KeyCode::Char('c') => Some(Action::ToggleSketchEngine),
                KeyCode::Char('s') => Some(Action::EnterParameters),
                KeyCode::Char('R') => Some(Action::RestartSketch),
                KeyCode::Char('e') => Some(Action::OpenInEditor),
                KeyCode::Char('b') | KeyCode::Char('w') => Some(Action::OpenInBrowser),
                KeyCode::Char('o') => Some(Action::EnterOpenAudio),
                _ => None,
            },
            InputMode::EditingParameter => match key.code {
                KeyCode::Esc => Some(Action::ExitParameters),
                KeyCode::Enter => Some(Action::CommitParameter),
                KeyCode::Backspace => Some(Action::ParameterBackspace),
                KeyCode::Char(ch) => Some(Action::ParameterInputChar(ch)),
                _ => None,
            },
            InputMode::EditingPrompt => match key.code {
                KeyCode::Esc => Some(Action::ExitPromptEdit),
                KeyCode::Enter => Some(Action::CommitPrompt),
                KeyCode::Backspace => Some(Action::PromptBackspace),
                KeyCode::Delete => Some(Action::PromptDelete),
                KeyCode::Left => Some(Action::PromptCursorLeft),
                KeyCode::Right => Some(Action::PromptCursorRight),
                KeyCode::Char(c) => Some(Action::PromptInputChar(c)),
                _ => None,
            },
            InputMode::OpeningAudio => match key.code {
                KeyCode::Esc => Some(Action::ExitOpenAudio),
                KeyCode::Enter => Some(Action::CommitOpenAudio),
                KeyCode::Backspace => Some(Action::AudioBackspace),
                KeyCode::Char(c) => Some(Action::AudioInputChar(c)),
                _ => None,
            },
            InputMode::Help => match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Enter => {
                    Some(Action::ToggleHelp)
                }
                _ => None,
            },
            InputMode::Versions => match key.code {
                KeyCode::Up | KeyCode::Char('k') => Some(Action::SelectPreviousVersion),
                KeyCode::Down | KeyCode::Char('j') => Some(Action::SelectNextVersion),
                KeyCode::Enter => Some(Action::RollbackToSelectedVersion),
                KeyCode::Esc | KeyCode::Char('v') | KeyCode::Char('q') => {
                    Some(Action::ToggleVersions)
                }
                _ => None,
            },
            InputMode::SelectSketchEngine => match key.code {
                KeyCode::Up | KeyCode::Char('k') => Some(Action::SelectSketchEngine(EngineId::P5)),
                KeyCode::Down | KeyCode::Char('j') => {
                    Some(Action::SelectSketchEngine(EngineId::Ascii))
                }
                KeyCode::Enter => Some(Action::ActivateSketchEngine),
                KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('q') => {
                    Some(Action::ToggleSketchEngine)
                }
                _ => None,
            },
            InputMode::SelectModel => match key.code {
                KeyCode::Up | KeyCode::Char('k') => Some(Action::SelectPreviousEngine),
                KeyCode::Down | KeyCode::Char('j') => Some(Action::SelectNextEngine),
                KeyCode::Enter => Some(Action::ActivateSelectedEngine),
                KeyCode::Esc | KeyCode::Char('m') | KeyCode::Char('q') => {
                    Some(Action::ToggleSelectModel)
                }
                _ => None,
            },
            InputMode::Tuning => match key.code {
                KeyCode::Up | KeyCode::Char('k') => Some(Action::TuningPrevParam),
                KeyCode::Down | KeyCode::Char('j') => Some(Action::TuningNextParam),
                KeyCode::Right | KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Char('l') => {
                    Some(Action::TuningIncreaseParam)
                }
                KeyCode::Left | KeyCode::Char('-') | KeyCode::Char('h') => {
                    Some(Action::TuningDecreaseParam)
                }
                KeyCode::Char('r') => Some(Action::TuningResetDefaults),
                KeyCode::Esc
                | KeyCode::Char('t')
                | KeyCode::Char('a')
                | KeyCode::Char('q')
                | KeyCode::Enter => Some(Action::ToggleTuningModal),
                _ => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::EngineKind;

    fn create_test_app(test_name: &str) -> App {
        let temp_dir = std::env::temp_dir().join(format!("grain_test_{}", test_name));
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let mut app = App::with_history_manager(HistoryManager::new(temp_dir));
        if let Some(idx) = app
            .state
            .engine
            .options
            .iter()
            .position(|o| o.kind == EngineKind::OfflineMock)
        {
            app.state.engine.active_index = idx;
            app.state.engine.selected_index = idx;
        }
        app
    }

    #[test]
    fn test_initial_state() {
        let app = create_test_app("initial_state");
        assert_eq!(app.state.mode, InputMode::Normal);
        assert!(!app.state.should_quit);
        assert!(!app.state.preview.is_playing);
    }

    #[test]
    fn test_quit_action() {
        let mut app = create_test_app("quit_action");
        app.update(Action::Quit);
        assert!(app.state.should_quit);
    }

    #[test]
    fn test_playback_toggle() {
        let mut app = create_test_app("playback_toggle");
        assert!(!app.state.preview.is_playing);
        app.update(Action::TogglePlayback);
        assert!(app.state.preview.is_playing);
        app.update(Action::TogglePlayback);
        assert!(!app.state.preview.is_playing);
    }

    #[test]
    fn test_prompt_editing_and_commit() {
        let mut app = create_test_app("prompt_commit");
        app.update(Action::EnterPromptEdit);
        assert_eq!(app.state.mode, InputMode::EditingPrompt);

        // Clear and type new text
        app.state.prompt.input_buffer.clear();
        app.state.prompt.cursor_position = 0;

        app.update(Action::PromptInputChar('n'));
        app.update(Action::PromptInputChar('e'));
        app.update(Action::PromptInputChar('w'));
        assert_eq!(app.state.prompt.input_buffer, "new");
        assert_eq!(app.state.prompt.cursor_position, 3);

        let mut next = app.update(Action::CommitPrompt);
        assert_eq!(app.state.mode, InputMode::Normal);
        assert_eq!(app.state.prompt.active_prompt, "new");
        assert_eq!(next, Some(Action::TriggerGenerate));

        while let Some(act) = next {
            next = app.update(act);
        }
        assert_eq!(app.state.prompt.current_version, 1);
        assert_eq!(app.state.preview.sketch_name, "sketch_v1");
    }

    #[test]
    fn test_load_audio_invalid_path() {
        let mut app = create_test_app("audio_invalid");
        app.update(Action::LoadAudio(PathBuf::from("non_existent_audio.wav")));
        assert_eq!(
            app.state.audio.path,
            Some(PathBuf::from("non_existent_audio.wav"))
        );
        match app.state.audio.status {
            AudioStatus::Error(_) => {}
            _ => panic!("Expected audio status to be Error for non-existent file"),
        }
    }

    #[test]
    fn test_tick_advances_frame_when_playing() {
        let mut app = create_test_app("tick_frame");
        app.state.preview.total_frames = 100;
        app.state.preview.current_frame = 0;
        app.state.preview.is_playing = true;

        let now = Instant::now();
        app.playback_clock = crate::preview::clock::PlaybackClock::new(now);
        app.advance_playback(now);
        app.advance_playback(now + std::time::Duration::from_nanos(16_666_667));
        assert_eq!(app.state.preview.current_frame, 1);
    }

    #[test]
    fn test_engine_selection_flow() {
        let mut app = create_test_app("engine_selection");
        assert_eq!(app.state.mode, InputMode::Normal);

        // Press 'm' to enter engine selection
        let action = app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));
        assert_eq!(action, Some(Action::ToggleSelectModel));
        app.update(action.unwrap());
        assert_eq!(app.state.mode, InputMode::SelectModel);

        // Move selection
        app.update(Action::SelectNextEngine);
        let selected_idx = app.state.engine.selected_index;

        // Activate selection
        app.update(Action::ActivateSelectedEngine);
        assert_eq!(app.state.mode, InputMode::Normal);
        assert_eq!(app.state.engine.active_index, selected_idx);
        assert!(
            app.state
                .status_message
                .unwrap()
                .contains("Switched AI Engine to:")
        );
    }
}
