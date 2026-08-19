use serde::{Deserialize, Serialize};
use crate::audio::AudioFeatures;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GrainContext {
    pub width: u32,
    pub height: u32,
    pub frame: usize,
    pub time: f64,
    pub seed: u64,
    pub audio: AudioFeatures,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeDiagnostic {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub stack: Option<String>,
}

impl std::fmt::Display for RuntimeDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let (Some(line), Some(col)) = (self.line, self.column) {
            write!(f, "[Line {}, Col {}] {}", line, col, self.message)
        } else if let Some(line) = self.line {
            write!(f, "[Line {}] {}", line, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalCell {
    pub symbol: String,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrameRenderResult {
    pub frame: usize,
    pub width: u32,
    pub height: u32,
    /// Encoded ASCII or pixel representation of the rendered frame
    pub ascii_art: Option<String>,
    /// High-fidelity TrueColor cell-by-cell RGB grid
    pub cells: Option<Vec<Vec<TerminalCell>>>,
    /// Visual entities / shapes drawn during the frame
    pub draw_commands_count: usize,
}
