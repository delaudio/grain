use std::path::PathBuf;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::action::Action;
use crate::state::{AudioStatus, GenerationStatus, GrainState, InputMode, PreviewStatus};

use crate::history::HistoryManager;

pub struct App {
    pub state: GrainState,
    pub history_manager: HistoryManager,
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

        if let Ok(history) = history_manager.load_history() {
            if !history.versions.is_empty() {
                state.prompt.total_versions = history.versions.len();
                state.prompt.current_version = history.active_version;
                state.versions.selected_index = history.active_version.saturating_sub(1);
                state.versions.history = history.clone();

                if let Some(active_meta) = history.versions.iter().find(|v| v.version == history.active_version) {
                    state.prompt.active_prompt = active_meta.prompt.clone();
                    state.preview.sketch_name = format!("sketch_v{}", active_meta.version);
                    if let Ok(code) = history_manager.load_sketch_content(&active_meta.sketch_file) {
                        state.preview.sketch_source = code;
                        state.preview.status = PreviewStatus::Ready;
                    }
                }
            }
        }

        Self {
            state,
            history_manager,
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

        self.state.audio.path = Some(path.clone());
        self.state.audio.status = AudioStatus::Loading;
        self.state.status_message = Some(format!("Analyzing audio: {}", file_name));

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
                if self.state.preview.is_playing {
                    if self.state.preview.total_frames > 0 {
                        self.state.preview.current_frame = (self.state.preview.current_frame + 1) % self.state.preview.total_frames;
                    }
                }
            }
            Action::Resize(w, h) => {
                self.state.terminal_size = (w, h);
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
                    && self.state.versions.selected_index + 1 < self.state.versions.history.versions.len()
                {
                    self.state.versions.selected_index += 1;
                }
            }
            Action::RollbackToSelectedVersion => {
                if let Some(v_meta) = self.state.versions.history.versions.get(self.state.versions.selected_index) {
                    return Some(Action::RollbackToVersion(v_meta.version));
                }
            }
            Action::RollbackToVersion(v) => {
                if let Some(v_meta) = self.state.versions.history.versions.iter().find(|m| m.version == v).cloned() {
                    if let Ok(content) = self.history_manager.load_sketch_content(&v_meta.sketch_file) {
                        self.state.preview.sketch_source = content;
                        self.state.prompt.active_prompt = v_meta.prompt.clone();
                        self.state.prompt.current_version = v_meta.version;
                        self.state.preview.sketch_name = format!("sketch_v{}", v_meta.version);
                        self.state.preview.status = PreviewStatus::Ready;
                        self.state.mode = InputMode::Normal;
                        self.state.versions.history.active_version = v_meta.version;
                        let _ = self.history_manager.save_history(&self.state.versions.history);
                        self.state.status_message = Some(format!("Rolled back to visual sketch v{}", v_meta.version));
                    }
                }
            }
            Action::TogglePlayback => {
                self.state.preview.is_playing = !self.state.preview.is_playing;
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
                if self.state.prompt.cursor_position < self.state.prompt.input_buffer.chars().count() {
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
                self.state.preview.is_playing = true;
                self.state.preview.seed = self.state.preview.seed.wrapping_add(1);
                self.state.prompt.generation_status = GenerationStatus::Generating;
                self.state.status_message = Some(format!(
                    "Generating audio-reactive visual via {}...",
                    self.state.engine.active_label()
                ));

                let prompt = self.state.prompt.active_prompt.clone();
                let seed = self.state.preview.seed;
                let is_revision = self.state.prompt.total_versions > 0;
                let current_sketch = self.state.preview.sketch_source.clone();

                let service = self.state.engine.create_service_for_active();
                let result = if is_revision {
                    service.revise_and_validate(&prompt, &current_sketch, seed)
                } else {
                    service.generate_and_validate(&prompt, seed)
                };

                return Some(Action::GenerationCompleted { result, prompt });
            }
            Action::GenerationCompleted { result, prompt } => {
                match result {
                    Ok(new_code) => {
                        let audio_hash = self.state.audio.path.as_ref().and_then(|p| {
                            crate::audio::get_cache_path(p, self.state.preview.fps)
                                .ok()
                                .and_then(|cp| cp.file_stem().map(|s| s.to_string_lossy().to_string()))
                        });

                        if let Ok(meta) = self.history_manager.record_new_version(
                            &prompt,
                            &new_code,
                            self.state.preview.seed,
                            "Grain Generator",
                            audio_hash.as_deref(),
                        ) {
                            if let Ok(hist) = self.history_manager.load_history() {
                                self.state.versions.history = hist;
                                self.state.versions.selected_index = self.state.versions.history.versions.len().saturating_sub(1);
                            }
                            self.state.prompt.current_version = meta.version;
                            self.state.prompt.total_versions = self.state.versions.history.versions.len();
                        } else {
                            self.state.prompt.total_versions += 1;
                            self.state.prompt.current_version = self.state.prompt.total_versions;
                        }

                        self.state.preview.sketch_source = new_code;
                        self.state.preview.sketch_name = format!("sketch_v{}", self.state.prompt.current_version);
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
            Action::SetStatusMessage(msg) => {
                self.state.status_message = Some(msg);
            }
        }
        None
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
                KeyCode::Char('v') => Some(Action::ToggleVersions),
                KeyCode::Char('m') => Some(Action::ToggleSelectModel),
                KeyCode::Char('o') => Some(Action::EnterOpenAudio),
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
            InputMode::SelectModel => match key.code {
                KeyCode::Up | KeyCode::Char('k') => Some(Action::SelectPreviousEngine),
                KeyCode::Down | KeyCode::Char('j') => Some(Action::SelectNextEngine),
                KeyCode::Enter => Some(Action::ActivateSelectedEngine),
                KeyCode::Esc | KeyCode::Char('m') | KeyCode::Char('q') => {
                    Some(Action::ToggleSelectModel)
                }
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
        if let Some(idx) = app.state.engine.options.iter().position(|o| o.kind == EngineKind::OfflineMock) {
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
        assert_eq!(app.state.audio.path, Some(PathBuf::from("non_existent_audio.wav")));
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

        app.update(Action::Tick);
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
        assert!(app.state.status_message.unwrap().contains("Switched AI Engine to:"));
    }
}
