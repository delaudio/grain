//! Bounded, persistent native-cell JavaScript runtime. No host module loader.

use std::time::{Duration, Instant};

use rquickjs::{Context, Ctx, Function, Module, Object, Persistent, Runtime};
use serde::Deserialize;

use super::engine::{CellFrame, EngineAdapter, EngineCapabilities, FrameOutput, OutputKind};
use super::{GrainContext, RuntimeDiagnostic};

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

fn caught<'js>(ctx: &Ctx<'js>, formatter: &Function<'js>) -> RuntimeDiagnostic {
    // Exception getters are untrusted code too. The caller's interrupt deadline
    // stays installed while the captured formatter attempts to read them.
    let encoded = formatter.call::<_, String>((ctx.catch(),));
    encoded
        .ok()
        .filter(|s| s.len() <= 16_384)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| diagnostic("ASCII JavaScript evaluation failed"))
}

#[derive(Deserialize)]
struct Response {
    frame: Option<CellFrame>,
    error: Option<RuntimeDiagnostic>,
}

pub struct AsciiSession {
    // QuickJS values must be dropped before their owning context and runtime.
    step: Persistent<Function<'static>>,
    js: Context,
    runtime: Runtime,
    seed: u64,
    dimensions: (u32, u32),
    last: Option<(GrainContext, u16, u16, f64, CellFrame)>,
    healthy: bool,
}

impl AsciiSession {
    pub fn new(source: &str, context: &GrainContext) -> Result<Self, RuntimeDiagnostic> {
        super::session::validate(context, 1, 1)?;
        if source.len() > 256 * 1024 {
            return Err(diagnostic("ASCII source exceeds 256 KiB"));
        }
        let runtime = Runtime::new().map_err(|e| diagnostic(e.to_string()))?;
        runtime.set_memory_limit(32 * 1024 * 1024);
        runtime.set_max_stack_size(256 * 1024);
        let deadline = Instant::now() + BUDGET;
        runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
        let js = Context::full(&runtime).map_err(|e| diagnostic(e.to_string()))?;
        let step = js.with(|ctx| -> Result<_, RuntimeDiagnostic> {
            let host: Object = ctx.eval(include_str!("js/ascii.js"))
                .map_err(|e| diagnostic(e.to_string()))?;
            let create: Function = host.get("create").map_err(|e| diagnostic(e.to_string()))?;
            let formatter: Function = host.get("formatError").map_err(|e| diagnostic(e.to_string()))?;
            let seed: Function = host.get("seed").map_err(|e| diagnostic(e.to_string()))?;
            seed.call::<_, ()>((context.seed.to_string(),)).map_err(|_| caught(&ctx, &formatter))?;
            // Appending an export does not shift user source line numbers. Both
            // script-style declarations and named ESM exports can define hooks.
            let source = format!("{source}\nexport const __grain_hooks = {{\nboot: typeof boot === 'undefined' ? undefined : boot,\npre: typeof pre === 'undefined' ? undefined : pre,\nmain: typeof main === 'undefined' ? undefined : main,\npost: typeof post === 'undefined' ? undefined : post,\nsettings: typeof settings === 'undefined' ? undefined : settings\n}};");
            let module = Module::declare(ctx.clone(), "sketch.ascii.js", source)
                .map_err(|_| caught(&ctx, &formatter))?;
            let (module, promise) = module.eval().map_err(|_| caught(&ctx, &formatter))?;
            match promise.result::<()>() {
                Some(Ok(())) => {}
                Some(Err(_)) => return Err(caught(&ctx, &formatter)),
                None => return Err(diagnostic("ASCII modules must initialize synchronously; top-level await is unsupported")),
            }
            let hooks: Object = module.get("__grain_hooks").map_err(|_| caught(&ctx, &formatter))?;
            let step: Function = create.call((hooks,)).map_err(|_| caught(&ctx, &formatter))?;
            Ok(Persistent::save(&ctx, step))
        });
        if Instant::now() >= deadline {
            return Err(diagnostic("ASCII initialization exceeded 250 ms"));
        }
        Ok(Self {
            step: step?,
            js,
            runtime,
            seed: context.seed,
            dimensions: (context.width, context.height),
            last: None,
            healthy: true,
        })
    }

    pub fn last_position(&self) -> Option<(usize, f64)> {
        self.last.as_ref().map(|(ctx, ..)| (ctx.frame, ctx.time))
    }

    pub fn render_cells(
        &mut self,
        context: &GrainContext,
        cols: u16,
        rows: u16,
        cell_aspect: f64,
    ) -> Result<CellFrame, RuntimeDiagnostic> {
        super::session::validate(context, cols, rows)?;
        if !cell_aspect.is_finite() || !(0.1..=4.0).contains(&cell_aspect) {
            return Err(diagnostic(
                "ASCII cell aspect must be finite and between 0.1 and 4",
            ));
        }
        if !self.healthy {
            return Err(diagnostic(
                "ASCII session failed; create a new session before rendering",
            ));
        }
        if context.seed != self.seed
            || (context.width, context.height) != self.dimensions
            || self
                .last_position()
                .is_some_and(|(frame, time)| context.frame < frame || context.time < time)
        {
            return Err(diagnostic(
                "ASCII seed, canvas resize, or backward seek requires a new session",
            ));
        }
        if let Some((previous, previous_cols, previous_rows, previous_aspect, frame)) = &self.last
            && previous == context
            && *previous_cols == cols
            && *previous_rows == rows
            && *previous_aspect == cell_aspect
        {
            return Ok(frame.clone());
        }
        self.healthy = false;
        let encoded = serde_json::to_string(&serde_json::json!({
            "context": context, "cols": cols, "rows": rows,
            "aspect": cell_aspect, "seedText": context.seed.to_string()
        }))
        .map_err(|e| diagnostic(e.to_string()))?;
        let deadline = Instant::now() + BUDGET;
        self.runtime
            .set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
        let encoded = self.js.with(|ctx| {
            self.step
                .clone()
                .restore(&ctx)?
                .call::<_, String>((encoded,))
        });
        if Instant::now() >= deadline {
            return Err(diagnostic("ASCII frame exceeded 250 ms"));
        }
        let encoded = encoded.map_err(|_| {
            diagnostic("ASCII execution failed or exceeded its memory/stack budget")
        })?;
        if encoded.len() > MAX_OUTPUT {
            return Err(diagnostic("ASCII output exceeds 4 MiB"));
        }
        let response: Response = serde_json::from_str(&encoded)
            .map_err(|_| diagnostic("Invalid ASCII frame response"))?;
        if let Some(error) = response.error {
            return Err(error);
        }
        let frame = response
            .frame
            .ok_or_else(|| diagnostic("ASCII engine returned no frame"))?;
        if frame.cols != cols || frame.rows != rows {
            return Err(diagnostic(
                "ASCII output dimensions differ from the requested grid",
            ));
        }
        FrameOutput::Cells(frame.clone()).validate()?;
        self.last = Some((context.clone(), cols, rows, cell_aspect, frame.clone()));
        self.healthy = true;
        Ok(frame)
    }
}

impl EngineAdapter for AsciiSession {
    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            output: OutputKind::Cells,
            persistent_state: true,
        }
    }

    fn render(
        &mut self,
        context: &GrainContext,
        cols: u16,
        rows: u16,
    ) -> Result<FrameOutput, RuntimeDiagnostic> {
        self.render_cells(context, cols, rows, 0.5)
            .map(FrameOutput::Cells)
    }
}
