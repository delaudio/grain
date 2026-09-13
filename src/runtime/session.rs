//! Persistent capability-free sketch session, owned by a rendering worker.
use std::time::{Duration, Instant};

use rquickjs::{Context, Function, Persistent, Runtime};
use serde::Deserialize;

use super::raster::{DrawCommand, half_block_cells, render_commands_on};
use super::{FrameRenderResult, GrainContext, RasterFrame, RuntimeDiagnostic};

const BUDGET: Duration = Duration::from_millis(250);
const MAX_OUTPUT: usize = 4 * 1024 * 1024;

fn diagnostic(message: impl Into<String>) -> RuntimeDiagnostic {
    RuntimeDiagnostic {
        message: message.into(),
        line: None,
        column: None,
        stack: None,
    }
}

pub(crate) fn validate(
    context: &GrainContext,
    cols: u16,
    rows: u16,
) -> Result<(), RuntimeDiagnostic> {
    if context.width == 0
        || context.height == 0
        || context.width > 4096
        || context.height > 4096
        || u64::from(context.width) * u64::from(context.height) > 4_194_304
        || cols == 0
        || rows == 0
        || usize::from(cols) * usize::from(rows) > 32_768
    {
        return Err(diagnostic("Invalid canvas or terminal dimensions"));
    }
    if !context.time.is_finite()
        || ![
            context.audio.amplitude,
            context.audio.low,
            context.audio.mid,
            context.audio.high,
        ]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err(diagnostic("Sketch context must contain finite numbers"));
    }
    Ok(())
}

#[derive(Deserialize)]
struct Response {
    success: bool,
    width: Option<u32>,
    height: Option<u32>,
    commands: Option<Vec<DrawCommand>>,
    error: Option<RuntimeDiagnostic>,
}

pub struct SketchSession {
    // Drop rooted JS values before their context and runtime, in this order.
    step: Persistent<Function<'static>>,
    js: Context,
    runtime: Runtime,
    canvas: Option<RasterFrame>,
    seed: u64,
    dimensions: (u32, u32),
    last_position: Option<(usize, f64)>,
    last_draw_count: usize,
    last_audio: Option<crate::audio::AudioFeatures>,
    healthy: bool,
}

impl SketchSession {
    pub fn new(source: &str, context: &GrainContext) -> Result<Self, RuntimeDiagnostic> {
        if source.len() > 256 * 1024 {
            return Err(diagnostic("Sketch source exceeds the 256 KiB limit"));
        }
        validate(context, 1, 1)?;
        let runtime =
            Runtime::new().map_err(|e| diagnostic(format!("Cannot initialize runtime: {e}")))?;
        runtime.set_memory_limit(32 * 1024 * 1024);
        runtime.set_max_stack_size(256 * 1024);
        let deadline = Instant::now() + BUDGET;
        runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
        let js = Context::full(&runtime)
            .map_err(|e| diagnostic(format!("Cannot initialize context: {e}")))?;
        let source = serde_json::to_string(source).map_err(|e| diagnostic(e.to_string()))?;
        let encoded = serde_json::to_string(context).map_err(|e| diagnostic(e.to_string()))?;
        let program = format!(
            "(() => {{\n{}\n{}\nreturn makeGrainSession({source}, {encoded});\n}})()",
            include_str!("js/runner.js"),
            include_str!("js/session.js")
        );
        let step = js.with(|ctx| {
            ctx.eval::<Function, _>(program)
                .map(|f| Persistent::save(&ctx, f))
        });
        if Instant::now() >= deadline {
            return Err(diagnostic(
                "Sketch initialization exceeded its 250 ms time limit",
            ));
        }
        let step = step.map_err(|e| {
            diagnostic(format!(
                "Sketch initialization failed (JavaScript, memory or stack limit): {e}"
            ))
        })?;
        Ok(Self {
            step,
            js,
            runtime,
            canvas: None,
            seed: context.seed,
            dimensions: (context.width, context.height),
            last_position: None,
            last_draw_count: 0,
            last_audio: None,
            healthy: true,
        })
    }

    pub fn last_position(&self) -> Option<(usize, f64)> {
        self.last_position
    }

    pub fn render(
        &mut self,
        context: &GrainContext,
        cols: u16,
        rows: u16,
    ) -> Result<FrameRenderResult, RuntimeDiagnostic> {
        validate(context, cols, rows)?;
        if !self.healthy {
            return Err(diagnostic("Failed sketch session must be reset"));
        }
        if context.seed != self.seed
            || (context.width, context.height) != self.dimensions
            || self
                .last_position
                .is_some_and(|(frame, time)| context.frame < frame || context.time < time)
        {
            return Err(diagnostic(
                "Seed, canvas change or backward seek requires a new sketch session",
            ));
        }
        // A paused resize only resamples the existing canvas, never re-runs draw.
        if self.last_position == Some((context.frame, context.time))
            && self.last_audio == Some(context.audio)
            && let Some(canvas) = &self.canvas
        {
            return Ok(frame_result(
                context.frame,
                canvas.clone(),
                self.last_draw_count,
                cols,
                rows,
            ));
        }
        // Any exception can leave user state partly mutated. Do not reuse it.
        self.healthy = false;
        let encoded = serde_json::to_string(context).map_err(|e| diagnostic(e.to_string()))?;
        let deadline = Instant::now() + BUDGET;
        self.runtime
            .set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
        let output = self.js.with(|ctx| {
            let step = self.step.clone().restore(&ctx)?;
            step.call::<_, String>((encoded,))
        });
        if Instant::now() >= deadline {
            return Err(diagnostic(
                "Sketch execution exceeded its 250 ms time limit",
            ));
        }
        let output = output.map_err(|e| {
            diagnostic(format!(
                "Sketch execution failed (JavaScript, memory or stack limit): {e}"
            ))
        })?;
        if output.len() > MAX_OUTPUT {
            return Err(diagnostic("Sketch output exceeds the 4 MiB limit"));
        }
        let response: Response = serde_json::from_str(&output)
            .map_err(|e| diagnostic(format!("Invalid sketch output: {e}")))?;
        if !response.success {
            return Err(response
                .error
                .unwrap_or_else(|| diagnostic("Sketch evaluation failed")));
        }
        let commands = response
            .commands
            .ok_or_else(|| diagnostic("Sketch returned no drawing commands"))?;
        let width = response
            .width
            .ok_or_else(|| diagnostic("Sketch returned no canvas width"))?;
        let height = response
            .height
            .ok_or_else(|| diagnostic("Sketch returned no canvas height"))?;
        let raster = render_commands_on(self.canvas.as_ref(), width, height, &commands)?;
        self.canvas = Some(raster.clone());
        self.last_position = Some((context.frame, context.time));
        self.last_draw_count = commands.len();
        self.last_audio = Some(context.audio);
        self.healthy = true;
        Ok(frame_result(
            context.frame,
            raster,
            commands.len(),
            cols,
            rows,
        ))
    }
}

fn frame_result(
    frame: usize,
    raster: RasterFrame,
    draw_commands_count: usize,
    cols: u16,
    rows: u16,
) -> FrameRenderResult {
    let cells = half_block_cells(&raster, cols, rows, 2.0, [0, 0, 0]);
    let ascii_art = cells
        .iter()
        .map(|row| row.iter().map(|c| c.symbol.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    FrameRenderResult {
        frame,
        width: raster.width,
        height: raster.height,
        ascii_art: Some(ascii_art),
        cells: Some(cells),
        raster: Some(raster),
        draw_commands_count,
        revision: 0,
    }
}
