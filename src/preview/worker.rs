//! A single owner for non-Send engine sessions, with one pending request/result.
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use crate::runtime::builtin::BuiltinEngineFactory;
use crate::runtime::engine::{
    CONTRACT_VERSION, EngineAdapter, EngineFactory, EngineFrame, EngineId, FrameOutput, OutputKind,
    ResetReason,
};
use crate::runtime::raster::half_block_cells;
use crate::runtime::{FrameRenderResult, GrainContext, RuntimeDiagnostic, TerminalCell};

#[derive(Clone)]
pub struct RenderRequest {
    pub revision: u64,
    pub lifecycle: u64,
    pub source: Arc<str>,
    pub context: GrainContext,
    pub cols: u16,
    pub rows: u16,
}

pub struct RenderCompletion {
    pub revision: u64,
    pub engine: EngineId,
    pub result: Result<FrameRenderResult, RuntimeDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputTarget {
    Terminal,
    Raw,
}

struct TerminalPreparation {
    sampled_cells: Option<Vec<Vec<TerminalCell>>>,
    ascii_art: String,
}

// Called only by the worker, outside the mailbox lock. Both area sampling and
// string assembly stay off the event/UI thread.
fn prepare_terminal(output: &FrameOutput, cols: u16, rows: u16) -> TerminalPreparation {
    let sampled_cells = match output {
        FrameOutput::Raster(raster) => Some(half_block_cells(raster, cols, rows, 2.0, [0, 0, 0])),
        FrameOutput::Cells(_) => None,
    };
    let cells = match output {
        FrameOutput::Cells(grid) => &grid.cells,
        FrameOutput::Raster(_) => sampled_cells.as_ref().expect("raster sampled above"),
    };
    let ascii_art = cells
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| cell.symbol.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    TerminalPreparation {
        sampled_cells,
        ascii_art,
    }
}

/// Engine output plus optional terminal presentation prepared on the worker.
/// Requests submitted to the Raw target never perform terminal sampling.
pub struct EngineCompletion {
    pub revision: u64,
    pub engine: EngineId,
    pub result: Result<EngineFrame, RuntimeDiagnostic>,
    pub cols: u16,
    pub rows: u16,
    pub draw_commands_count: usize,
    target: OutputTarget,
    terminal: Option<TerminalPreparation>,
}

impl EngineCompletion {
    pub fn target(&self) -> OutputTarget {
        self.target
    }

    /// Move a prepared terminal result without sampling or assembling strings.
    /// Raw requests must be resubmitted for Terminal presentation instead of
    /// silently doing expensive work on the consuming thread.
    pub fn into_terminal(self) -> RenderCompletion {
        let result = self.result.and_then(|frame| {
            let prepared = self.terminal.ok_or_else(|| RuntimeDiagnostic {
                message: "Raw output has no terminal preparation; submit a Terminal request".into(),
                line: None,
                column: None,
                stack: None,
            })?;
            let (width, height, cells, raster) = match frame.output {
                FrameOutput::Cells(grid) => {
                    (u32::from(grid.cols), u32::from(grid.rows), grid.cells, None)
                }
                FrameOutput::Raster(raster) => {
                    let cells = prepared
                        .sampled_cells
                        .expect("terminal raster prepared by worker");
                    (raster.width, raster.height, cells, Some(raster))
                }
            };
            Ok(FrameRenderResult {
                frame: frame.frame,
                width,
                height,
                ascii_art: Some(prepared.ascii_art),
                cells: Some(cells),
                raster,
                draw_commands_count: self.draw_commands_count,
                revision: frame.revision,
            })
        });
        RenderCompletion {
            revision: self.revision,
            engine: self.engine,
            result,
        }
    }
}

#[derive(Default)]
struct Mailbox {
    pending: Option<(EngineId, RenderRequest, OutputTarget)>,
    completed: Option<EngineCompletion>,
    stopping: bool,
}

#[derive(Clone, PartialEq)]
struct Identity {
    engine: EngineId,
    source: Arc<str>,
    seed: u64,
    width: u32,
    height: u32,
    lifecycle: u64,
}

struct Active {
    identity: Identity,
    adapter: Box<dyn EngineAdapter>,
    last_position: (usize, f64),
}

fn reset_reason(
    active: Option<&Active>,
    key: &Identity,
    context: &GrainContext,
) -> Option<ResetReason> {
    let Some(active) = active else {
        return Some(ResetReason::SourceChanged);
    };
    let previous = &active.identity;
    if previous.engine != key.engine {
        Some(ResetReason::EngineChanged)
    } else if previous.source != key.source {
        Some(ResetReason::SourceChanged)
    } else if previous.seed != key.seed {
        Some(ResetReason::SeedChanged)
    } else if (previous.width, previous.height) != (key.width, key.height) {
        Some(ResetReason::CanvasResized)
    } else if previous.lifecycle != key.lifecycle {
        Some(ResetReason::Restart)
    } else if context.frame < active.last_position.0 || context.time < active.last_position.1 {
        Some(ResetReason::Seek)
    } else if !active.adapter.capabilities().persistent_state {
        Some(ResetReason::Restart)
    } else {
        None
    }
}

fn render(
    adapter: &mut dyn EngineAdapter,
    request: &RenderRequest,
) -> Result<FrameOutput, RuntimeDiagnostic> {
    let output = adapter.render(&request.context, request.cols, request.rows)?;
    output.validate()?;
    let matches_capabilities = match &output {
        FrameOutput::Cells(grid) => {
            adapter.capabilities().output == OutputKind::Cells
                && grid.cols == request.cols
                && grid.rows == request.rows
        }
        FrameOutput::Raster(_) => adapter.capabilities().output == OutputKind::Raster,
    };
    if !matches_capabilities {
        return Err(RuntimeDiagnostic {
            message: "Engine output does not match its capabilities or requested cell grid".into(),
            line: None,
            column: None,
            stack: None,
        });
    }
    Ok(output)
}

pub struct PreviewWorker {
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl PreviewWorker {
    pub fn new() -> std::io::Result<Self> {
        Self::with_factory(Arc::new(BuiltinEngineFactory))
    }

    /// Factories are trusted host integrations, not user-code plugins. Built-in
    /// adapters enforce JS budgets; fake factories make scheduling testable.
    pub fn with_factory(factory: Arc<dyn EngineFactory>) -> std::io::Result<Self> {
        let shared = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let worker_shared = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("grain-preview".into())
            .spawn(move || {
                let mut active: Option<Active> = None;
                let mut failed: Option<(
                    Identity,
                    crate::runtime::parameters::Parameters,
                    RuntimeDiagnostic,
                )> = None;
                loop {
                    let (engine, request, target) = {
                        let (lock, wake) = &*worker_shared;
                        let mut mailbox = lock.lock().unwrap_or_else(|e| e.into_inner());
                        while mailbox.pending.is_none() && !mailbox.stopping {
                            mailbox = wake.wait(mailbox).unwrap_or_else(|e| e.into_inner());
                        }
                        if mailbox.stopping {
                            break;
                        }
                        mailbox
                            .pending
                            .take()
                            .expect("pending request checked under lock")
                    };
                    let key = Identity {
                        engine,
                        source: Arc::clone(&request.source),
                        seed: request.context.seed,
                        width: request.context.width,
                        height: request.context.height,
                        lifecycle: request.lifecycle,
                    };
                    let preflight = crate::runtime::session::validate(
                        &request.context,
                        request.cols,
                        request.rows,
                    );
                    let cache_failure = preflight.is_ok();
                    let reason =
                        reset_reason(active.as_ref(), &key, &request.context).or_else(|| {
                            failed
                                .as_ref()
                                .filter(|(identity, params, _)| {
                                    identity == &key && params != &request.context.params
                                })
                                .map(|_| ResetReason::ParametersChanged)
                        });
                    let result = if let Err(error) = preflight {
                        Err(error)
                    } else if let Some((_, _, error)) =
                        failed.as_ref().filter(|(identity, params, _)| {
                            identity == &key && params == &request.context.params
                        })
                    {
                        Err(error.clone())
                    } else if let Some(reason) = reason {
                        // Do not discard the previous valid session on a failed switch.
                        match factory
                            .create(engine, &request.source, &request.context, reason)
                            .and_then(|mut adapter| {
                                let output = render(adapter.as_mut(), &request)?;
                                Ok((adapter, output))
                            }) {
                            Ok((adapter, output)) => {
                                active = Some(Active {
                                    identity: key.clone(),
                                    adapter,
                                    last_position: (request.context.frame, request.context.time),
                                });
                                failed = None;
                                Ok(output)
                            }
                            Err(error) => Err(error),
                        }
                    } else {
                        let active = active.as_mut().expect("active session checked above");
                        render(active.adapter.as_mut(), &request).inspect(|_| {
                            active.last_position = (request.context.frame, request.context.time);
                        })
                    };
                    let draw_commands_count = if result.is_ok() {
                        active
                            .as_ref()
                            .map_or(0, |active| active.adapter.draw_commands_count())
                    } else {
                        0
                    };
                    if cache_failure && let Err(error) = &result {
                        failed = Some((key, request.context.params.clone(), error.clone()));
                    }
                    let result = result.map(|output| EngineFrame {
                        engine,
                        contract_version: CONTRACT_VERSION,
                        revision: request.revision,
                        frame: request.context.frame,
                        time: request.context.time,
                        output,
                    });
                    let terminal = if target == OutputTarget::Terminal {
                        result.as_ref().ok().map(|frame| {
                            prepare_terminal(&frame.output, request.cols, request.rows)
                        })
                    } else {
                        None
                    };
                    let mut mailbox = worker_shared.0.lock().unwrap_or_else(|e| e.into_inner());
                    if mailbox.stopping {
                        break;
                    }
                    if mailbox
                        .pending
                        .as_ref()
                        .is_none_or(|(next_engine, next, next_target)| {
                            *next_engine == engine
                                && *next_target == target
                                && next.revision == request.revision
                                && next.cols == request.cols
                                && next.rows == request.rows
                        })
                    {
                        mailbox.completed = Some(EngineCompletion {
                            engine,
                            revision: request.revision,
                            result,
                            cols: request.cols,
                            rows: request.rows,
                            draw_commands_count,
                            target,
                            terminal,
                        });
                    }
                }
            })?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Legacy callers remain p5 until the independent sketch selector is wired.
    pub fn submit(&self, request: RenderRequest) {
        self.submit_for_engine(EngineId::P5, request);
    }

    pub fn submit_for_engine(&self, engine: EngineId, request: RenderRequest) {
        self.submit_target(engine, request, OutputTarget::Terminal);
    }

    /// Image backends request untouched raster/native output, with no cell conversion.
    pub fn submit_raw_for_engine(&self, engine: EngineId, request: RenderRequest) {
        self.submit_target(engine, request, OutputTarget::Raw);
    }

    fn submit_target(&self, engine: EngineId, request: RenderRequest, target: OutputTarget) {
        let mut mailbox = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        if !mailbox.stopping {
            if mailbox.completed.as_ref().is_some_and(|result| {
                result.engine != engine
                    || result.target != target
                    || result.revision != request.revision
                    || result.cols != request.cols
                    || result.rows != request.rows
            }) {
                mailbox.completed = None;
            }
            mailbox.pending = Some((engine, request, target));
            self.shared.1.notify_one();
        }
    }

    pub fn take_engine_completed(&self) -> Option<EngineCompletion> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .completed
            .take()
    }

    pub fn take_completed(&self) -> Option<RenderCompletion> {
        self.take_engine_completed()
            .map(EngineCompletion::into_terminal)
    }
}

impl Drop for PreviewWorker {
    fn drop(&mut self) {
        {
            let mut mailbox = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
            mailbox.stopping = true;
            mailbox.pending = None;
            self.shared.1.notify_one();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
