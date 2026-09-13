//! Engine-neutral output and lifecycle contract. Sketches never supply frame identity.

use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthChar;

use super::{GrainContext, RasterFrame, RuntimeDiagnostic, TerminalCell};

pub const CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineId {
    #[default]
    P5,
    Ascii,
}

impl EngineId {
    pub const fn label(self) -> &'static str {
        match self {
            Self::P5 => "p5",
            Self::Ascii => "ASCII",
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::P5 => 0,
            Self::Ascii => 1,
        }
    }

    pub const fn contract_name(self) -> &'static str {
        match self {
            Self::P5 => "grain-p5-v1",
            Self::Ascii => "grain-ascii-v1",
        }
    }
}

pub const fn default_contract_version() -> u32 {
    CONTRACT_VERSION
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellFrame {
    pub cols: u16,
    pub rows: u16,
    /// Row-major cells. Each symbol occupies exactly one terminal column.
    pub cells: Vec<Vec<TerminalCell>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "frame", rename_all = "lowercase")]
pub enum FrameOutput {
    Cells(CellFrame),
    Raster(RasterFrame),
}

impl FrameOutput {
    /// Check bounded shape before an output reaches a terminal backend.
    /// Ambiguous-width characters use Unicode's narrow (non-CJK) width policy.
    pub fn validate(&self) -> Result<(), RuntimeDiagnostic> {
        let invalid = |message: &str| RuntimeDiagnostic {
            message: message.to_owned(),
            line: None,
            column: None,
            stack: None,
        };
        match self {
            Self::Cells(frame) => {
                if frame.cols == 0
                    || frame.rows == 0
                    || usize::from(frame.cols) * usize::from(frame.rows) > 32_768
                    || frame.cells.len() != usize::from(frame.rows)
                    || frame
                        .cells
                        .iter()
                        .any(|row| row.len() != usize::from(frame.cols))
                {
                    return Err(invalid(
                        "Invalid cell frame dimensions (maximum 32768 cells)",
                    ));
                }
                for cell in frame.cells.iter().flatten() {
                    let mut chars = cell.symbol.chars();
                    if !matches!(chars.next(), Some(ch) if !ch.is_control() && ch.width() == Some(1))
                        || chars.next().is_some()
                    {
                        return Err(invalid(
                            "Cell symbols must be one printable, single-column Unicode scalar",
                        ));
                    }
                }
            }
            Self::Raster(frame) => {
                let pixels = u64::from(frame.width) * u64::from(frame.height);
                if frame.width == 0
                    || frame.height == 0
                    || frame.width > 4096
                    || frame.height > 4096
                    || pixels > 4_194_304
                    || frame.rgba.len() as u64 != pixels * 4
                {
                    return Err(invalid("Invalid raster frame dimensions or RGBA length"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineFrame {
    pub engine: EngineId,
    pub contract_version: u32,
    pub revision: u64,
    pub frame: usize,
    pub time: f64,
    pub output: FrameOutput,
}

/// Lifecycle events are selected by the coordinator, not by user JavaScript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetReason {
    ParametersChanged,
    SourceChanged,
    EngineChanged,
    SeedChanged,
    Seek,
    Restart,
    CanvasResized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    Cells,
    Raster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineCapabilities {
    pub output: OutputKind,
    pub persistent_state: bool,
}

/// Trusted host adapters own a bounded execution environment. Implementations
/// are instantiated on the preview worker: a JavaScript session need not be Send.
pub trait EngineAdapter {
    fn capabilities(&self) -> EngineCapabilities;

    fn draw_commands_count(&self) -> usize {
        0
    }

    fn render(
        &mut self,
        context: &GrainContext,
        cols: u16,
        rows: u16,
    ) -> Result<FrameOutput, RuntimeDiagnostic>;
}

pub trait EngineFactory: Send + Sync {
    fn create(
        &self,
        engine: EngineId,
        source: &str,
        context: &GrainContext,
        reason: ResetReason,
    ) -> Result<Box<dyn EngineAdapter>, RuntimeDiagnostic>;
}
