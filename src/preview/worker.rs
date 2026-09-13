//! A single owner for the non-Send JS runtime, with one pending request/result.
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use crate::runtime::session::SketchSession;
use crate::runtime::{FrameRenderResult, GrainContext, RuntimeDiagnostic};

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
    pub result: Result<FrameRenderResult, RuntimeDiagnostic>,
}

#[derive(Default)]
struct Mailbox {
    pending: Option<RenderRequest>,
    completed: Option<RenderCompletion>,
    stopping: bool,
}

type Identity = (Arc<str>, u64, u32, u32, u64);

pub struct PreviewWorker {
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl PreviewWorker {
    pub fn new() -> std::io::Result<Self> {
        let shared = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let worker_shared = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("grain-preview".into())
            .spawn(move || {
                let mut active: Option<(Identity, SketchSession)> = None;
                let mut failed: Option<(Identity, RuntimeDiagnostic)> = None;
                loop {
                    let request = {
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
                    let key = (
                        Arc::clone(&request.source),
                        request.context.seed,
                        request.context.width,
                        request.context.height,
                        request.lifecycle,
                    );
                    let preflight = crate::runtime::session::validate(
                        &request.context,
                        request.cols,
                        request.rows,
                    );
                    let cache_failure = preflight.is_ok();
                    let reset = active.as_ref().is_none_or(|(identity, session)| {
                        identity != &key
                            || session.last_position().is_some_and(|(frame, time)| {
                                request.context.frame < frame || request.context.time < time
                            })
                    });
                    let result = if let Err(error) = preflight {
                        // Invalid viewport/context does not execute or poison user state.
                        Err(error)
                    } else if let Some((_, error)) =
                        failed.as_ref().filter(|(identity, _)| identity == &key)
                    {
                        Err(error.clone())
                    } else if reset {
                        // Provisional session: preserve the previous one until the
                        // replacement has produced a usable first frame.
                        match SketchSession::new(&request.source, &request.context).and_then(
                            |mut session| {
                                let result =
                                    session.render(&request.context, request.cols, request.rows)?;
                                Ok((session, result))
                            },
                        ) {
                            Ok((session, result)) => {
                                active = Some((key.clone(), session));
                                failed = None;
                                Ok(result)
                            }
                            Err(error) => Err(error),
                        }
                    } else {
                        active
                            .as_mut()
                            .expect("active session checked above")
                            .1
                            .render(&request.context, request.cols, request.rows)
                    };
                    let result = result.map(|mut result| {
                        result.revision = request.revision;
                        result
                    });
                    if cache_failure && let Err(error) = &result {
                        failed = Some((key, error.clone()));
                    }
                    let mut mailbox = worker_shared.0.lock().unwrap_or_else(|e| e.into_inner());
                    if mailbox.stopping {
                        break;
                    }
                    // Never publish results from an obsolete source/viewport/seek.
                    if mailbox
                        .pending
                        .as_ref()
                        .is_none_or(|next| next.revision == request.revision)
                    {
                        mailbox.completed = Some(RenderCompletion {
                            revision: request.revision,
                            result,
                        });
                    }
                }
            })?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    pub fn submit(&self, request: RenderRequest) {
        let mut mailbox = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        if !mailbox.stopping {
            mailbox.pending = Some(request);
            self.shared.1.notify_one();
        }
    }

    pub fn take_completed(&self) -> Option<RenderCompletion> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .completed
            .take()
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
