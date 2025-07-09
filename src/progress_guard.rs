use std::{collections::HashSet, sync::Arc, time::Duration};

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
    rx: Receiver<Ready>,
}

impl ProgressGuard {
    /// Start guard.
    pub(crate) fn start(tasks: &mut JoinSet<Result<()>>, client: Arc<LspClient>) -> Self {
        let (tx, rx) = channel(Ready {
            init: false,
            progress: true,
        });

        // HACK: there doesn't seem to be a way to know what progress tokens
        // to expect initially, so we just give the language server some time to hit us with a few
        let tx_captured = tx.clone();
        tasks.spawn(async move {
            debug!("wait for initial language server warm-up");
            tokio::time::sleep(Duration::from_secs(2)).await;
            tx_captured.send_modify(|rdy| rdy.init = true);
            debug!("wait for initial language server warm-up");

            // never return
            futures::future::pending::<()>().await;
            Ok(())
        });

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
                    if rdy.progress != new_ready {
                        rdy.progress = new_ready;
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
        self.rx.clone().wait_for(|rdy| rdy.ready()).await.ok();
    }
}

#[derive(Debug)]
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
