//! Bounded, antialiased raster drawing and area-filtered terminal sampling.
use std::time::{Duration, Instant};

use serde::Deserialize;
use tiny_skia::{FillRule, LineCap, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, Transform};

use super::{RasterFrame, RuntimeDiagnostic, TerminalCell};

const MAX_PIXELS: u64 = 4_194_304;
const MAX_COMMANDS: usize = 4096;
const MAX_VERTICES: usize = 4096;
const MAX_CELLS: usize = 32_768;

type Rgba = [f32; 4];

#[derive(Debug, Deserialize)]
pub struct DrawStyle {
    matrix: [f32; 6],
    fill: Option<Rgba>,
    stroke: Option<Rgba>,
    weight: f32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DrawCommand {
    Background {
        color: Rgba,
    },
    Clear,
    Ellipse {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        style: DrawStyle,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        style: DrawStyle,
    },
    Path {
        vertices: Vec<[f32; 2]>,
        close: bool,
        style: DrawStyle,
    },
}

fn error(message: &str) -> RuntimeDiagnostic {
    RuntimeDiagnostic {
        message: message.into(),
        line: None,
        column: None,
        stack: None,
    }
}

fn valid(values: &[f32]) -> bool {
    values
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
}

fn paint(color: Rgba) -> Result<Paint<'static>, RuntimeDiagnostic> {
    if !valid(&color) {
        return Err(error("Non-finite or excessive drawing color"));
    }
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paint.set_color_rgba8(
        color[0].clamp(0.0, 255.0).round() as u8,
        color[1].clamp(0.0, 255.0).round() as u8,
        color[2].clamp(0.0, 255.0).round() as u8,
        (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    );
    Ok(paint)
}

fn draw(pixmap: &mut Pixmap, path: Path, style: &DrawStyle) -> Result<(), RuntimeDiagnostic> {
    if !valid(&style.matrix) || !style.weight.is_finite() || !(0.0..=4096.0).contains(&style.weight)
    {
        return Err(error("Invalid transform or stroke weight (maximum 4096)"));
    }
    let m = style.matrix;
    let transform = Transform::from_row(m[0], m[1], m[2], m[3], m[4], m[5]);
    // Reject excessive transformed coordinates before entering the rasterizer.
    let bounds = path.bounds();
    for (x, y) in [
        (bounds.left(), bounds.top()),
        (bounds.right(), bounds.top()),
        (bounds.left(), bounds.bottom()),
        (bounds.right(), bounds.bottom()),
    ] {
        if !valid(&[m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]) {
            return Err(error("Transformed geometry exceeds drawing limits"));
        }
    }
    if let Some(fill) = style.fill {
        pixmap.fill_path(&path, &paint(fill)?, FillRule::Winding, transform, None);
    }
    if let Some(stroke) = style.stroke.filter(|_| style.weight > 0.0) {
        let stroke_style = Stroke {
            width: style.weight,
            line_cap: LineCap::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint(stroke)?, &stroke_style, transform, None);
    }
    Ok(())
}

/// A command deadline is checked between draws. This is a bounded soft budget,
/// not a preemptive OS-level deadline inside an individual raster operation.
pub fn render_commands_on(
    previous: Option<&RasterFrame>,
    width: u32,
    height: u32,
    commands: &[DrawCommand],
) -> Result<RasterFrame, RuntimeDiagnostic> {
    if width == 0
        || height == 0
        || width > 4096
        || height > 4096
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(error(
            "Raster canvas exceeds dimension or 4 megapixel limit",
        ));
    }
    if commands.len() > MAX_COMMANDS {
        return Err(error("Sketch exceeds the 4096 drawing command limit"));
    }
    let mut pixmap =
        Pixmap::new(width, height).ok_or_else(|| error("Cannot allocate raster canvas"))?;
    if let Some(previous) = previous.filter(|p| p.width == width && p.height == height) {
        if previous.rgba.len() != pixmap.data().len() {
            return Err(error("Invalid previous raster canvas"));
        }
        for (target, source) in pixmap
            .data_mut()
            .chunks_exact_mut(4)
            .zip(previous.rgba.chunks_exact(4))
        {
            let alpha = u32::from(source[3]);
            for c in 0..3 {
                target[c] = ((u32::from(source[c]) * alpha + 127) / 255) as u8;
            }
            target[3] = source[3];
        }
    }
    let deadline = Instant::now() + Duration::from_millis(250);

    for command in commands {
        if Instant::now() >= deadline {
            return Err(error("Raster rendering exceeded its 250 ms time limit"));
        }
        match command {
            DrawCommand::Clear => pixmap.data_mut().fill(0),
            DrawCommand::Background { color } => {
                let rect = Rect::from_xywh(0.0, 0.0, width as f32, height as f32)
                    .ok_or_else(|| error("Invalid background dimensions"))?;
                pixmap.fill_rect(rect, &paint(*color)?, Transform::identity(), None);
            }
            DrawCommand::Ellipse { x, y, w, h, style } => {
                if !valid(&[*x, *y, *w, *h]) {
                    return Err(error("Invalid ellipse geometry"));
                }
                let Some(rect) =
                    Rect::from_xywh(x - w.abs() / 2.0, y - h.abs() / 2.0, w.abs(), h.abs())
                else {
                    continue;
                };
                let mut builder = PathBuilder::new();
                builder.push_oval(rect);
                if let Some(path) = builder.finish() {
                    draw(&mut pixmap, path, style)?;
                }
            }
            DrawCommand::Rect { x, y, w, h, style } => {
                if !valid(&[*x, *y, *w, *h]) {
                    return Err(error("Invalid rectangle geometry"));
                }
                let Some(rect) = Rect::from_xywh(x.min(x + w), y.min(y + h), w.abs(), h.abs())
                else {
                    continue;
                };
                draw(&mut pixmap, PathBuilder::from_rect(rect), style)?;
            }
            DrawCommand::Path {
                vertices,
                close,
                style,
            } => {
                if vertices.len() > MAX_VERTICES || vertices.iter().any(|point| !valid(point)) {
                    return Err(error("Invalid path or 4096 vertex limit exceeded"));
                }
                if let Some(first) = vertices.first() {
                    let mut builder = PathBuilder::new();
                    builder.move_to(first[0], first[1]);
                    for point in &vertices[1..] {
                        builder.line_to(point[0], point[1]);
                    }
                    if *close {
                        builder.close();
                    }
                    if let Some(path) = builder.finish() {
                        draw(&mut pixmap, path, style)?;
                    }
                }
            }
        }
    }
    if Instant::now() >= deadline {
        return Err(error("Raster rendering exceeded its 250 ms time limit"));
    }
    // tiny-skia stores premultiplied RGBA; expose straight alpha to consumers.
    let mut rgba = pixmap.data().to_vec();
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        if alpha > 0 {
            for component in &mut pixel[..3] {
                *component = ((u32::from(*component) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    Ok(RasterFrame {
        width,
        height,
        rgba,
    })
}

/// Two independently colored samples per terminal cell. `cell_aspect` is
/// physical cell height / width (2.0 unless terminal metrics say otherwise).
/// The whole viewport is returned, with centered letterboxing and no borders.
pub fn half_block_cells(
    frame: &RasterFrame,
    cols: u16,
    rows: u16,
    cell_aspect: f64,
    backdrop: [u8; 3],
) -> Vec<Vec<TerminalCell>> {
    let pixels = u64::from(frame.width) * u64::from(frame.height);
    if cols == 0
        || rows == 0
        || usize::from(cols) * usize::from(rows) > MAX_CELLS
        || pixels == 0
        || pixels > MAX_PIXELS
        || frame.rgba.len() as u64 != pixels * 4
        || !cell_aspect.is_finite()
        || !(0.25..=8.0).contains(&cell_aspect)
    {
        return Vec::new();
    }
    let width = f64::from(frame.width);
    let height = f64::from(frame.height);
    let pixel_aspect = cell_aspect / 2.0;
    let scale = (f64::from(cols) / width).min(f64::from(rows) * 2.0 * pixel_aspect / height);
    let left = (f64::from(cols) - width * scale) / 2.0;
    let top = (f64::from(rows) * 2.0 - height * scale / pixel_aspect) / 2.0;
    let sample = |x: usize, y: usize| -> [u8; 3] {
        let x0 = (x as f64 - left) / scale;
        let x1 = (x as f64 + 1.0 - left) / scale;
        let y0 = (y as f64 - top) * pixel_aspect / scale;
        let y1 = (y as f64 + 1.0 - top) * pixel_aspect / scale;
        let total = (x1 - x0) * (y1 - y0);
        let mut sum = backdrop.map(|c| f64::from(c) * total);
        for sy in y0.max(0.0).floor() as u32..y1.min(height).max(0.0).ceil() as u32 {
            let wy = (y1.min(f64::from(sy) + 1.0) - y0.max(f64::from(sy))).max(0.0);
            for sx in x0.max(0.0).floor() as u32..x1.min(width).max(0.0).ceil() as u32 {
                let wx = (x1.min(f64::from(sx) + 1.0) - x0.max(f64::from(sx))).max(0.0);
                let offset = (sy as usize * frame.width as usize + sx as usize) * 4;
                let pixel = &frame.rgba[offset..offset + 4];
                let weight = wx * wy * f64::from(pixel[3]) / 255.0;
                for c in 0..3 {
                    sum[c] += (f64::from(pixel[c]) - f64::from(backdrop[c])) * weight;
                }
            }
        }
        sum.map(|v| (v / total).round().clamp(0.0, 255.0) as u8)
    };
    (0..usize::from(rows))
        .map(|row| {
            (0..usize::from(cols))
                .map(|col| {
                    let upper = sample(col, row * 2);
                    let lower = sample(col, row * 2 + 1);
                    TerminalCell {
                        symbol: "\u{2580}".into(),
                        r: upper[0],
                        g: upper[1],
                        b: upper[2],
                        background: Some(lower),
                    }
                })
                .collect()
        })
        .collect()
}

/// One-shot compatibility path used for generation validation.
pub fn render_commands(
    width: u32,
    height: u32,
    commands: &[DrawCommand],
) -> Result<RasterFrame, RuntimeDiagnostic> {
    render_commands_on(None, width, height, commands)
}
