//! Bounded, side-effect-free encoding for the iTerm2 inline image protocol.
//!
//! The terminal owner must serialize these packets with Ratatui writes. Encoding
//! belongs on the preview worker, never on the input or presentation thread.

use crate::runtime::RasterFrame;
use ratatui::layout::Rect;

pub const MAX_IMAGE_PIXELS: usize = 160_000;
pub const MAX_PNG_BYTES: usize = 750_000;
pub const MAX_PACKET_BYTES: usize = 1_048_576;

/// A bounded PNG, composited against the terminal preview's black background.
pub struct InlineImage {
    png: Vec<u8>,
}

impl InlineImage {
    /// Area-average only the presentation image, never the engine canvas. Color
    /// is composited before filtering to avoid transparent-color fringes.
    pub fn from_canvas(frame: &RasterFrame) -> Result<Self, String> {
        let pixels = u64::from(frame.width) * u64::from(frame.height);
        if frame.width == 0
            || frame.height == 0
            || frame.width > 4096
            || frame.height > 4096
            || pixels > 4_194_304
            || frame.rgba.len() as u64 != pixels * 4
        {
            return Err("Invalid source canvas for iTerm2".into());
        }
        if pixels <= MAX_IMAGE_PIXELS as u64 {
            return Self::from_raster(frame);
        }
        let scale = (MAX_IMAGE_PIXELS as f64 / pixels as f64).sqrt();
        let width = ((f64::from(frame.width) * scale).floor() as u32).max(1);
        let height = ((f64::from(frame.height) * scale).floor() as u32).max(1);
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        let dx = f64::from(frame.width) / f64::from(width);
        let dy = f64::from(frame.height) / f64::from(height);
        for y in 0..height {
            let top = f64::from(y) * dy;
            let bottom = f64::from(y + 1) * dy;
            for x in 0..width {
                let left = f64::from(x) * dx;
                let right = f64::from(x + 1) * dx;
                let mut color = [0.0; 3];
                for sy in top.floor() as u32..(bottom.ceil() as u32).min(frame.height) {
                    let wy = bottom.min(f64::from(sy + 1)) - top.max(f64::from(sy));
                    for sx in left.floor() as u32..(right.ceil() as u32).min(frame.width) {
                        let weight = wy * (right.min(f64::from(sx + 1)) - left.max(f64::from(sx)));
                        let offset = (sy as usize * frame.width as usize + sx as usize) * 4;
                        let alpha = f64::from(frame.rgba[offset + 3]) / 255.0;
                        for (channel, sum) in color.iter_mut().enumerate() {
                            *sum += f64::from(frame.rgba[offset + channel]) * alpha * weight;
                        }
                    }
                }
                rgba.extend(color.map(|sum| (sum / (dx * dy)).round().clamp(0.0, 255.0) as u8));
                rgba.push(255);
            }
        }
        Self::from_raster(&RasterFrame {
            width,
            height,
            rgba,
        })
    }

    /// The caller chooses resolution before rendering/encoding. Oversized frames
    /// are rejected rather than silently changing their geometry here.
    pub fn from_raster(frame: &RasterFrame) -> Result<Self, String> {
        let pixels = (frame.width as usize)
            .checked_mul(frame.height as usize)
            .ok_or("Image dimensions overflow")?;
        if frame.width == 0
            || frame.height == 0
            || frame.width > 4096
            || frame.height > 4096
            || pixels > MAX_IMAGE_PIXELS
            || frame.rgba.len() != pixels * 4
        {
            return Err("Invalid or oversized iTerm2 raster".into());
        }
        let mut opaque = Vec::with_capacity(frame.rgba.len());
        for pixel in frame.rgba.chunks_exact(4) {
            let alpha = u16::from(pixel[3]);
            for channel in &pixel[..3] {
                opaque.push(((u16::from(*channel) * alpha + 127) / 255) as u8);
            }
            opaque.push(255);
        }
        let size = tiny_skia::IntSize::from_wh(frame.width, frame.height)
            .ok_or("Invalid PNG dimensions")?;
        let pixmap =
            tiny_skia::Pixmap::from_vec(opaque, size).ok_or("Invalid PNG pixel storage")?;
        let png = pixmap.encode_png().map_err(|error| error.to_string())?;
        if png.len() > MAX_PNG_BYTES {
            return Err("PNG exceeds the iTerm2 transmission budget".into());
        }
        Ok(Self { png })
    }

    pub fn png_bytes(&self) -> &[u8] {
        &self.png
    }

    /// Save/restore the cursor and use cell units so iTerm2 owns Retina scaling.
    /// Leave a row below the image to avoid scrolling at the terminal bottom.
    pub fn packet(&self, area: Rect, screen: Rect) -> Result<Vec<u8>, String> {
        validate_area(area, screen)?;
        let mut packet = format!(
            "\x1b7\x1b[{};{}H\x1b]1337;File=inline=1;size={};width={};height={};preserveAspectRatio=1:",
            u32::from(area.y) + 1, u32::from(area.x) + 1,
            self.png.len(), area.width, area.height,
        ).into_bytes();
        append_base64(&self.png, &mut packet);
        packet.extend_from_slice(b"\x07\x1b8");
        if packet.len() > MAX_PACKET_BYTES {
            return Err("Image packet exceeds the iTerm2 transmission budget".into());
        }
        Ok(packet)
    }
}

/// Explicit cell replacement is needed even when Ratatui's text buffer has not
/// changed. The owner must invalidate/redraw its buffer after clearing an image.
pub fn clear_packet(area: Rect, screen: Rect) -> Result<Vec<u8>, String> {
    validate_area(area, screen)?;
    let mut packet = b"\x1b7\x1b[0;40m".to_vec();
    let spaces = vec![b' '; usize::from(area.width)];
    for row in area.y..area.bottom() {
        packet.extend_from_slice(
            format!("\x1b[{};{}H", u32::from(row) + 1, u32::from(area.x) + 1).as_bytes(),
        );
        packet.extend_from_slice(&spaces);
    }
    packet.extend_from_slice(b"\x1b8");
    Ok(packet)
}

fn validate_area(area: Rect, screen: Rect) -> Result<(), String> {
    let right = u32::from(area.x) + u32::from(area.width);
    let bottom = u32::from(area.y) + u32::from(area.height);
    if screen.x != 0
        || screen.y != 0
        || area.width == 0
        || area.height == 0
        || right > u32::from(screen.width)
        || bottom >= u32::from(screen.height)
        || usize::from(area.width) * usize::from(area.height) > 32_768
    {
        return Err("Image rectangle must fit the screen with a spare bottom row".into());
    }
    Ok(())
}

fn append_base64(bytes: &[u8], out: &mut Vec<u8>) {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        out.push(ALPHABET[usize::from(a >> 2)]);
        out.push(ALPHABET[usize::from(((a & 3) << 4) | (b >> 4))]);
        out.push(if chunk.len() > 1 {
            ALPHABET[usize::from(((b & 15) << 2) | (c >> 6))]
        } else {
            b'='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[usize::from(c & 63)]
        } else {
            b'='
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_vectors_and_packet_framing() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            let mut encoded = Vec::new();
            append_base64(input.as_bytes(), &mut encoded);
            assert_eq!(encoded, expected.as_bytes());
        }
        let image = InlineImage::from_raster(&RasterFrame {
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 255],
        })
        .unwrap();
        let packet = image
            .packet(Rect::new(2, 3, 10, 5), Rect::new(0, 0, 80, 24))
            .unwrap();
        assert!(packet.starts_with(b"\x1b7\x1b[4;3H\x1b]1337;File=inline=1;size="));
        assert!(packet.ends_with(b"\x07\x1b8"));
        assert!(!packet.contains(&b'\n'));
        assert!(packet.len() <= MAX_PACKET_BYTES);
    }

    #[test]
    fn png_preserves_opaque_channels_and_composites_alpha_on_black() {
        let image = InlineImage::from_raster(&RasterFrame {
            width: 2,
            height: 1,
            rgba: vec![10, 20, 30, 255, 200, 100, 50, 128],
        })
        .unwrap();
        let decoded = tiny_skia::Pixmap::decode_png(image.png_bytes()).unwrap();
        assert_eq!(decoded.data(), &[10, 20, 30, 255, 100, 50, 25, 255]);
    }

    #[test]
    fn invalid_frames_and_scrolling_rectangles_are_rejected() {
        for frame in [
            RasterFrame {
                width: 0,
                height: 1,
                rgba: vec![],
            },
            RasterFrame {
                width: 1,
                height: 1,
                rgba: vec![0],
            },
            RasterFrame {
                width: 800,
                height: 600,
                rgba: vec![],
            },
        ] {
            assert!(InlineImage::from_raster(&frame).is_err());
        }
        let screen = Rect::new(0, 0, 80, 24);
        for area in [
            Rect::new(0, 0, 0, 1),
            Rect::new(0, 23, 1, 1),
            Rect::new(79, 0, 2, 1),
        ] {
            assert!(clear_packet(area, screen).is_err());
        }
        assert_eq!(
            clear_packet(Rect::new(2, 3, 2, 2), screen).unwrap(),
            b"\x1b7\x1b[0;40m\x1b[4;3H  \x1b[5;3H  \x1b8"
        );
    }
}
