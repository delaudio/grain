//! Built-in factories are trusted host code; user sources run inside bounded JS sessions.

use super::ascii::AsciiSession;
use super::engine::{
    EngineAdapter, EngineCapabilities, EngineFactory, EngineId, FrameOutput, OutputKind,
    ResetReason,
};
use super::session::SketchSession;
use super::{GrainContext, RuntimeDiagnostic};

pub struct BuiltinEngineFactory;

impl EngineFactory for BuiltinEngineFactory {
    fn create(
        &self,
        engine: EngineId,
        source: &str,
        context: &GrainContext,
        _reason: ResetReason,
    ) -> Result<Box<dyn EngineAdapter>, RuntimeDiagnostic> {
        match engine {
            EngineId::P5 => Ok(Box::new(P5Adapter {
                session: SketchSession::new(source, context)?,
                draw_commands_count: 0,
            })),
            EngineId::Ascii => Ok(Box::new(AsciiSession::new(source, context)?)),
        }
    }
}

struct P5Adapter {
    session: SketchSession,
    draw_commands_count: usize,
}

impl EngineAdapter for P5Adapter {
    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            output: OutputKind::Raster,
            persistent_state: true,
        }
    }

    fn draw_commands_count(&self) -> usize {
        self.draw_commands_count
    }

    fn render(
        &mut self,
        context: &GrainContext,
        _cols: u16,
        _rows: u16,
    ) -> Result<FrameOutput, RuntimeDiagnostic> {
        let (raster, count) = self.session.render_raster(context)?;
        self.draw_commands_count = count;
        Ok(FrameOutput::Raster(raster))
    }
}
