use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use crate::runtime::FrameRenderResult;

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

impl PreviewBackend for AnsiPreviewBackend {
    fn name(&self) -> &'static str {
        "ANSI / Half-Block High Fidelity"
    }

    fn render_frame(&self, result: &FrameRenderResult, area: Rect) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let max_lines = area.height.saturating_sub(2) as usize;

        if let Some(ref cells) = result.cells {
            for row in cells.iter().take(max_lines) {
                let mut spans = Vec::new();
                for cell in row {
                    let is_bright = cell.r > 180 || cell.g > 180 || cell.b > 180;
                    let mut style = Style::default().fg(Color::Rgb(cell.r, cell.g, cell.b));
                    if is_bright {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    spans.push(Span::styled(cell.symbol.clone(), style));
                }
                lines.push(Line::from(spans));
            }
        } else if let Some(ref ascii) = result.ascii_art {
            for row in ascii.lines().take(max_lines) {
                let mut spans = Vec::new();
                for ch in row.chars() {
                    let style = match ch {
                        '@' | '%' => Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                        '#' | '*' => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        '+' | '=' => Style::default().fg(Color::Yellow),
                        '-' | ':' | '.' => Style::default().fg(Color::DarkGray),
                        _ => Style::default().fg(Color::Reset),
                    };
                    spans.push(Span::styled(ch.to_string(), style));
                }
                lines.push(Line::from(spans));
            }
        }

        lines
    }
}

#[derive(Debug, Default, Clone)]
pub struct RattyTerminalBackend;

impl RattyTerminalBackend {
    pub fn new() -> Self {
        Self
    }
}

impl PreviewBackend for RattyTerminalBackend {
    fn name(&self) -> &'static str {
        "Ratty Terminal Graphics"
    }

    fn render_frame(&self, result: &FrameRenderResult, area: Rect) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let max_lines = area.height.saturating_sub(2) as usize;

        if let Some(ref cells) = result.cells {
            for row in cells.iter().take(max_lines) {
                let mut spans = Vec::new();
                for cell in row {
                    let is_bright = cell.r > 200 || cell.g > 200 || cell.b > 200;
                    let mut style = Style::default().fg(Color::Rgb(cell.r, cell.g, cell.b));
                    if is_bright {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    spans.push(Span::styled(cell.symbol.clone(), style));
                }
                lines.push(Line::from(spans));
            }
        } else if let Some(ref ascii) = result.ascii_art {
            for row in ascii.lines().take(max_lines) {
                let mut spans = Vec::new();
                for ch in row.chars() {
                    let style = match ch {
                        '@' => Style::default().fg(Color::Rgb(255, 100, 200)).add_modifier(Modifier::BOLD),
                        '%' => Style::default().fg(Color::Rgb(220, 80, 255)).add_modifier(Modifier::BOLD),
                        '#' => Style::default().fg(Color::Rgb(80, 220, 255)).add_modifier(Modifier::BOLD),
                        '*' => Style::default().fg(Color::Rgb(100, 255, 200)),
                        '+' => Style::default().fg(Color::Rgb(255, 220, 100)),
                        '=' => Style::default().fg(Color::Rgb(200, 180, 80)),
                        '-' => Style::default().fg(Color::Rgb(120, 120, 160)),
                        ':' | '.' => Style::default().fg(Color::Rgb(60, 60, 90)),
                        _ => Style::default().fg(Color::Reset),
                    };
                    spans.push(Span::styled(ch.to_string(), style));
                }
                lines.push(Line::from(spans));
            }
        }

        lines
    }
}
