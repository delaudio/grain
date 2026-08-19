use std::sync::Arc;
use ratatui::layout::Rect;
use ratatui::text::Line;
use crate::audio::AudioFeatures;
use crate::preview::backend::{AnsiPreviewBackend, PreviewBackend, RattyTerminalBackend};
use crate::runtime::{evaluate_frame, FrameRenderResult, GrainContext, RuntimeDiagnostic};

pub struct PreviewEngine {
    backend: Arc<dyn PreviewBackend>,
}

impl Default for PreviewEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewEngine {
    pub fn new() -> Self {
        // Detect environment or default to Ratty/high-fidelity backend
        let backend: Arc<dyn PreviewBackend> = if std::env::var("GRAIN_ANSI_ONLY").is_ok() {
            Arc::new(AnsiPreviewBackend::new())
        } else {
            Arc::new(RattyTerminalBackend::new())
        };

        Self { backend }
    }

    #[allow(dead_code)]
    pub fn with_backend(backend: Arc<dyn PreviewBackend>) -> Self {
        Self { backend }
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    pub fn render_frame(
        &self,
        source: &str,
        frame: usize,
        fps: u32,
        seed: u64,
        audio: AudioFeatures,
        area: Rect,
    ) -> Result<(FrameRenderResult, Vec<Line<'static>>), RuntimeDiagnostic> {
        let time = frame as f64 / fps.max(1) as f64;
        let cols = area.width.saturating_sub(4).max(10);
        let rows = area.height.saturating_sub(4).max(4);

        let ctx = GrainContext {
            width: 800,
            height: 600,
            frame,
            time,
            seed,
            audio,
        };

        let result = evaluate_frame(source, &ctx, cols, rows)?;
        let lines = self.backend.render_frame(&result, area);

        Ok((result, lines))
    }
}
