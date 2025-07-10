use std::{path::PathBuf, process::Stdio, sync::Arc};

use anyhow::{Context, Result};
use clap::Parser;
use init::init_lsp;
use io_intercept::{BoxRead, BoxWrite, ReadFork, WriteFork};
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
mod io_intercept;
mod logging;
mod mcp;
mod progress_guard;

#[derive(Debug, Parser)]
struct Args {
    #[clap(long)]
    workspace: PathBuf,

    /// Intercept IO to/from language server and MCP client for debugging.
    ///
    /// Dumps are stored in separate files in the provided directory.
    #[clap(long)]
    intercept_io: Option<PathBuf>,

    /// Logging config.
    #[clap(flatten)]
    logging_cfg: LoggingCLIConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    setup_logging(args.logging_cfg).context("logging setup")?;

    let mut tasks = JoinSet::new();

    let workspace = args
        .workspace
        .canonicalize()
        .context("canonicalize workspace path")?;

    if let Some(intercept_io) = &args.intercept_io {
        tokio::fs::create_dir_all(intercept_io)
            .await
            .context("create directories for IO interception")?;
    }

    let stderr = if let Some(intercept_io) = &args.intercept_io {
        Stdio::from(
            tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(intercept_io.join("lsp.stderr.txt"))
                .await
                .context("open stderr log file for language server")?
                .into_std()
                .await,
        )
    } else {
        Stdio::inherit()
    };

    let mut child = Command::new("rust-analyzer")
        .current_dir(&args.workspace)
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .context("cannot spawn language server")?;

    let stdin = Box::pin(child.stdin.take().expect("just initialized")) as BoxWrite;
    let stdout = Box::pin(child.stdout.take().expect("just initialized")) as BoxRead;
    let (stdin, stdout) = if let Some(intercept_io) = &args.intercept_io {
        let stdin =
            Box::pin(WriteFork::new(stdin, intercept_io, "lsp.stdin.txt", &mut tasks).await?) as _;
        let stdout =
            Box::pin(ReadFork::new(stdout, intercept_io, "lsp.stdout.txt", &mut tasks).await?) as _;
        (stdin, stdout)
    } else {
        (stdin, stdout)
    };
    let (tx, rx) = io_transport(stdin, stdout);
    let client = Arc::new(LspClient::new(tx, rx));

    let progress_guard = ProgressGuard::start(&mut tasks, Arc::clone(&client));

    let (stdin, stdout) = stdio();
    let stdin = Box::pin(stdin) as BoxRead;
    let stdout = Box::pin(stdout) as BoxWrite;
    let (stdin, stdout) = if let Some(intercept_io) = &args.intercept_io {
        let stdin =
            Box::pin(ReadFork::new(stdin, intercept_io, "mcp.stdin.txt", &mut tasks).await?) as _;
        let stdout =
            Box::pin(WriteFork::new(stdout, intercept_io, "mcp.stdout.txt", &mut tasks).await?)
                as _;
        (stdin, stdout)
    } else {
        (stdin, stdout)
    };

    let mut res = tokio::select! {
        res = main_inner(client, progress_guard, workspace, stdin, stdout) => {
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
    stdin: BoxRead,
    stdout: BoxWrite,
) -> Result<()> {
    init_lsp(&client, &workspace).await.context("init lsp")?;

    let service = CodeExplorer::new(progress_guard, workspace)
        .serve((stdin, stdout))
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
