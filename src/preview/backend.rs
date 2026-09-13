use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::runtime::raster::half_block_cells;
use crate::runtime::{FrameRenderResult, TerminalCell};

pub trait PreviewBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn render_frame(&self, result: &FrameRenderResult, area: Rect) -> Vec<Line<'static>>;
}

#[derive(Debug, Default, Clone)]
pub struct AnsiPreviewBackend;

impl AnsiPreviewBackend {
    pub fn new() -> Self {
        Self
    }
}

pub fn cell_lines(cells: &[Vec<TerminalCell>], area: Rect) -> Vec<Line<'static>> {
    cells
        .iter()
        .take(usize::from(area.height))
        .map(|row| {
            Line::from(
                row.iter()
                    .take(usize::from(area.width))
                    .map(|cell| {
                        let mut style = Style::default().fg(Color::Rgb(cell.r, cell.g, cell.b));
                        if let Some([r, g, b]) = cell.background {
                            style = style.bg(Color::Rgb(r, g, b));
                        }
                        Span::styled(cell.symbol.clone(), style)
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

impl PreviewBackend for AnsiPreviewBackend {
    fn name(&self) -> &'static str {
        "TrueColor half-block"
    }

    fn render_frame(&self, result: &FrameRenderResult, area: Rect) -> Vec<Line<'static>> {
        if let Some(raster) = &result.raster {
            let cells = half_block_cells(raster, area.width, area.height, 2.0, [0, 0, 0]);
            cell_lines(&cells, area)
        } else if let Some(cells) = &result.cells {
            // Native cell engines bypass raster sampling entirely.
            cell_lines(cells, area)
        } else {
            result
                .ascii_art
                .as_deref()
                .unwrap_or("")
                .lines()
                .take(usize::from(area.height))
                .map(|line| {
                    Line::raw(
                        line.chars()
                            .take(usize::from(area.width))
                            .collect::<String>(),
                    )
                })
                .collect()
        }
    }
}

/// Compatibility alias for callers of the old placeholder backend.
#[derive(Debug, Default, Clone)]
pub struct RattyTerminalBackend;

impl RattyTerminalBackend {
    pub fn new() -> Self {
        Self
    }
}

impl PreviewBackend for RattyTerminalBackend {
    fn name(&self) -> &'static str {
        "TrueColor half-block"
    }
    fn render_frame(&self, result: &FrameRenderResult, area: Rect) -> Vec<Line<'static>> {
        AnsiPreviewBackend.render_frame(result, area)
    }
}
