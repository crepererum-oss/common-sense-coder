use std::{collections::HashSet, ops::Deref, sync::Arc};

use anyhow::{Context, Result, ensure};
use lsp_client::LspClient;
use lsp_types::{NumberOrString, ProgressParamsValue, WorkDoneProgress, notification::Progress};
use tokio::{
    sync::watch::{Receiver, channel},
    task::JoinSet,
};
use tracing::{debug, info};

use crate::ProgrammingLanguageQuirks;

/// Allows to wait for in-progress language server tasks.
#[derive(Debug, Clone)]
pub(crate) struct ProgressGuard {
    rx: Receiver<Ready>,
    client: Arc<LspClient>,
}

impl ProgressGuard {
    /// Start guard.
    pub(crate) fn start(
        tasks: &mut JoinSet<Result<()>>,
        quirks: &Arc<dyn ProgrammingLanguageQuirks>,
        client: Arc<LspClient>,
    ) -> Self {
        let (tx, rx) = channel(Ready {
            init: false,
            progress: true,
        });

        // HACK: there doesn't seem to be a way to know what progress tokens
        // to expect initially, so we just have a hard-coded list
        let mut init_parts = quirks.init_progress_parts();

        let client_captured = Arc::clone(&client);
        tasks.spawn(async move {
            let client = client_captured;
            let mut subscription = client
                .subscribe_to_method::<Progress>()
                .await
                .context("subscribe to 'progress'")?;

            let mut running = HashSet::new();

            while let Some(res) = subscription.next().await {
                let progress = res.context("receive progress")?;
                let ProgressParamsValue::WorkDone(work_done_progress) = progress.value;

                match work_done_progress {
                    WorkDoneProgress::Begin(_) => {
                        ensure!(
                            running.insert(progress.token.clone()),
                            "Progress double start: {:?}",
                            progress.token,
                        );
                        if let NumberOrString::String(token) = &progress.token {
                            init_parts.remove(token);
                        }
                        debug!(phase="start", token=?progress.token, running=running.len(), to_init=init_parts.len(), "progress");
                    }
                    WorkDoneProgress::Report(_) => {}
                    WorkDoneProgress::End(_) => {
                        ensure!(
                            running.remove(&progress.token),
                            "Progress end without start: {:?}",
                            progress.token,
                        );
                        debug!(phase="end", token=?progress.token, running=running.len(), to_init=init_parts.len(), "progress");
                    }
                }

                let new_rdy = Ready {
                    init: init_parts.is_empty(),
                    progress: running.is_empty(),
                };
                tx.send_if_modified(|rdy| {
                    if rdy != &new_rdy {
                        let flag_changed = rdy.ready() != new_rdy.ready();

                        *rdy = new_rdy;

                        if flag_changed {
                            info!(progrss=rdy.progress, init=rdy.init, ready=rdy.ready(), "ready changed");
                        } else {
                            debug!(progrss=rdy.progress, init=rdy.init, ready=rdy.ready(), "ready changed");
                        }

                        true
                    } else {
                        false
                    }
                });
            }

            Result::Ok(())
        });

        Self { rx, client }
    }

    /// Wait for all outstanding tasks.
    pub(crate) async fn wait(&self) -> Guard<'_> {
        // accept errors during shutdown
        self.rx.clone().wait_for(|rdy| rdy.ready()).await.ok();

        Guard {
            process_guard: self,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Guard<'a> {
    process_guard: &'a ProgressGuard,
}

impl Deref for Guard<'_> {
    type Target = LspClient;

    fn deref(&self) -> &Self::Target {
        self.process_guard.client.as_ref()
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Ready {
    init: bool,
    progress: bool,
}

impl Ready {
    fn ready(&self) -> bool {
        let Self { init, progress } = self;
        *init && *progress
    }
}
