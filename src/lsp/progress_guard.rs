use std::{collections::HashSet, ops::Deref, sync::Arc};

use anyhow::{Context, Result, ensure};
use futures::Stream;
use lsp_client::LspClient;
use lsp_types::{
    NumberOrString, ProgressParamsValue, WorkDoneProgress, WorkDoneProgressBegin,
    WorkDoneProgressEnd, WorkDoneProgressReport, notification::Progress,
};
use tokio::{
    sync::watch::{Receiver, channel},
    task::JoinSet,
};
use tokio_stream::wrappers::WatchStream;
use tracing::{debug, info};

use crate::ProgrammingLanguageQuirks;

/// Allows to wait for in-progress language server tasks.
#[derive(Debug, Clone)]
pub(crate) struct ProgressGuard {
    rx_rdy: Receiver<Ready>,
    rx_evt: Receiver<String>,
    client: Arc<LspClient>,
}

impl ProgressGuard {
    /// Start guard.
    pub(crate) fn start(
        tasks: &mut JoinSet<Result<()>>,
        quirks: &Arc<dyn ProgrammingLanguageQuirks>,
        client: Arc<LspClient>,
    ) -> Self {
        let (tx_rdy, rx_rdy) = channel(Ready {
            init: false,
            progress: true,
        });
        let (tx_evt, rx_evt) = channel(String::new());

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

                let evt = match work_done_progress {
                    WorkDoneProgress::Begin(WorkDoneProgressBegin{title, message, percentage, ..}) => {
                        ensure!(
                            running.insert(progress.token.clone()),
                            "Progress double start: {:?}",
                            progress.token,
                        );
                        if let NumberOrString::String(token) = &progress.token {
                            init_parts.remove(token);
                        }
                        debug!(phase="start", token=?progress.token, running=running.len(), to_init=init_parts.len(), "progress");

                        format_event(&progress.token, "start", Some(title), message, percentage)
                    }
                    WorkDoneProgress::Report(WorkDoneProgressReport { message, percentage, .. }) => {
                        format_event(&progress.token, "progress", None, message, percentage)
                    }
                    WorkDoneProgress::End(WorkDoneProgressEnd { message }) => {
                        ensure!(
                            running.remove(&progress.token),
                            "Progress end without start: {:?}",
                            progress.token,
                        );
                        debug!(phase="end", token=?progress.token, running=running.len(), to_init=init_parts.len(), "progress");
                        format_event(&progress.token, "end", None, message, None)
                    }
                };
                tx_evt.send(evt).ok();

                let new_rdy = Ready {
                    init: init_parts.is_empty(),
                    progress: running.is_empty(),
                };
                tx_rdy.send_if_modified(|rdy| {
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

        Self {
            rx_rdy,
            rx_evt,
            client,
        }
    }

    /// A stream of progress events.
    pub(crate) fn events(&self) -> impl Stream<Item = String> {
        WatchStream::from_changes(self.rx_evt.clone())
    }

    /// Wait for all outstanding tasks.
    pub(crate) async fn wait(&self) -> Guard<'_> {
        // accept errors during shutdown
        self.rx_rdy.clone().wait_for(|rdy| rdy.ready()).await.ok();

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

fn format_event(
    token: &NumberOrString,
    phase: &'static str,
    title: Option<String>,
    message: Option<String>,
    percentage: Option<u32>,
) -> String {
    let mut parts = vec![phase.to_owned()];
    if let NumberOrString::String(token) = token {
        parts.push(token.clone());
    }
    if let Some(title) = title {
        parts.push(title);
    }
    if let Some(message) = message {
        parts.push(message);
    }
    if let Some(percantage) = percentage {
        parts.push(format!("{percantage}%"))
    }
    parts.join(" ")
}
