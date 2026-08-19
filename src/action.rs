use std::path::PathBuf;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    Tick,
    Resize(u16, u16),
    ToggleHelp,
    ToggleVersions,
    TogglePlayback,
    EnterPromptEdit,
    ExitPromptEdit,
    PromptInputChar(char),
    PromptBackspace,
    PromptDelete,
    PromptCursorLeft,
    PromptCursorRight,
    CommitPrompt,
    TriggerGenerate,
    EnterOpenAudio,
    ExitOpenAudio,
    AudioInputChar(char),
    AudioBackspace,
    CommitOpenAudio,
    LoadAudio(PathBuf),
    SetStatusMessage(String),
}
