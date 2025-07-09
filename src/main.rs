use std::{path::PathBuf, process::Stdio, sync::Arc};

use anyhow::{Context, Result};
use clap::Parser;
use init::init_lsp;
use logging::{LoggingCLIConfig, setup_logging};
use lsp_client::{LspClient, transport::io_transport};
use mcp::CodeExplorer;
use progress_guard::ProgressGuard;
use rmcp::{ServiceExt, transport::stdio};
use tokio::{
    process::Command,
    task::{JoinError, JoinSet},
};
use tracing::{info, warn};

mod init;
mod logging;
mod mcp;
mod progress_guard;

#[derive(Debug, Parser)]
struct Args {
    #[clap(long)]
    workspace: PathBuf,

    /// Logging config.
    #[clap(flatten)]
    logging_cfg: LoggingCLIConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    setup_logging(args.logging_cfg).context("logging setup")?;

    let workspace = args
        .workspace
        .canonicalize()
        .context("canonicalize workspace path")?;

    // TODO: stderr to log file
    let mut child = Command::new("rust-analyzer")
        .current_dir(&args.workspace)
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("cannot spawn language server")?;

    let stdin = child.stdin.take().expect("just initialized");
    let stdout = child.stdout.take().expect("just initialized");
    let (tx, rx) = io_transport(stdin, stdout);
    let client = Arc::new(LspClient::new(tx, rx));

    let mut tasks = JoinSet::new();
    let progress_guard = ProgressGuard::start(&mut tasks, Arc::clone(&client));

    let mut res = tokio::select! {
        res = main_inner(client, progress_guard, workspace) => {
            res.context("main")
        }
        res = tasks.join_next(), if !tasks.is_empty() => {
            flatten_task_result(res.expect("checked that there are tasks"))
        }
    };

    if let Err(e) = &res {
        warn!(%e, "system failed");
    }

    info!("shutdown server");
    tasks.abort_all();
    while let Some(res2) = tasks.join_next().await {
        let res2 = match res2 {
            Ok(inner) => Ok(inner),
            Err(e) if e.is_cancelled() => Ok(Ok(())),
            Err(e) => Err(e),
        };
        let res2 = flatten_task_result(res2);
        res = res.and(res2);
    }

    res = res.and(child.kill().await.context("terminate language server"));

    info!("shutdown complete");
    res
}

async fn main_inner(
    client: Arc<LspClient>,
    progress_guard: ProgressGuard,
    workspace: PathBuf,
) -> Result<()> {
    init_lsp(&client, &workspace).await.context("init lsp")?;

    let service = CodeExplorer::new(client, progress_guard, workspace)
        .serve(stdio())
        .await
        .context("set up code explorer service")?;
    service.waiting().await?;

    Ok(())
}

fn flatten_task_result(res: Result<Result<()>, JoinError>) -> Result<()> {
    match res {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e).context("task"),
        Err(e) => Err(e).context("join"),
    }
}
