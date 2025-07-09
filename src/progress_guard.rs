use std::{collections::HashSet, sync::Arc};

use anyhow::{Context, Result, ensure};
use lsp_client::LspClient;
use lsp_types::{ProgressParamsValue, WorkDoneProgress, notification::Progress};
use tokio::{
    sync::watch::{Receiver, channel},
    task::JoinSet,
};
use tracing::debug;

/// Allows to wait for in-progress language server tasks.
#[derive(Debug, Clone)]
pub(crate) struct ProgressGuard {
    rx: Receiver<bool>,
}

impl ProgressGuard {
    /// Start guard.
    pub(crate) fn start(tasks: &mut JoinSet<Result<()>>, client: Arc<LspClient>) -> Self {
        let (tx, rx) = channel(true);

        tasks.spawn(async move {
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
                        debug!(phase="start", token=?progress.token, running=running.len(), "progress");
                    }
                    WorkDoneProgress::Report(_) => {}
                    WorkDoneProgress::End(_) => {
                        ensure!(
                            running.remove(&progress.token),
                            "Progress end without start: {:?}",
                            progress.token,
                        );
                        debug!(phase="end", token=?progress.token, running=running.len(), "progress");
                    }
                }

                let new_ready = running.is_empty();
                tx.send_if_modified(|rdy| {
                    if *rdy != new_ready {
                        *rdy = new_ready;
                        debug!(ready=new_ready, "ready change");
                        true
                    } else {
                        false
                    }
                });
            }

            Result::Ok(())
        });

        Self { rx }
    }

    /// Wait for all outstanding tasks.
    pub(crate) async fn wait(&self) {
        // accept errors during shutdown
        self.rx.clone().wait_for(|rdy| *rdy).await.ok();
    }
}
