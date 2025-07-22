use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use clap::Parser;
use constants::{REVISION, VERSION, VERSION_STRING};
use io_intercept::{BoxRead, BoxWrite, ReadFork, WriteFork};
use lang::{ProgrammingLanguage, ProgrammingLanguageQuirks};
use logging::{LoggingCLIConfig, setup_logging};
use lsp::{
    init::{init_lsp, spawn_lsp},
    progress_guard::ProgressGuard,
};
use lsp_client::LspClient;
use mcp::CodeExplorer;
use rmcp::{ServiceExt, transport::stdio};
use tokio::task::{JoinError, JoinSet};
use tracing::{info, warn};

// used in integration tests
#[cfg(test)]
use assert_cmd as _;
#[cfg(test)]
use insta as _;
#[cfg(test)]
use predicates as _;
#[cfg(test)]
use tempfile as _;

mod constants;
mod io_intercept;
mod lang;
mod logging;
mod lsp;
mod mcp;

/// Provides a "common sense" interface for a language model via Model Context Provider (MCP).
///
/// The data is retrieved from a language server (via LSP).
#[derive(Debug, Parser)]
#[command(version = VERSION_STRING)]
struct Args {
    /// Workspace location, i.e. the root of the project.
    #[clap(long, env = "COMMON_SENSE_CODER_WORKSPACE")]
    workspace: PathBuf,

    /// Intercept IO to/from language server and MCP client for debugging.
    ///
    /// Dumps are stored in separate files in the provided directory.
    #[clap(long, env = "COMMON_SENSE_CODER_INTERCEPT_IO")]
    intercept_io: Option<PathBuf>,

    /// Programming language.
    #[clap(long, default_value = "rust")]
    programming_language: ProgrammingLanguage,

    /// Logging config.
    #[clap(flatten)]
    logging_cfg: LoggingCLIConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    let dotenv_path = match dotenvy::dotenv() {
        Ok(path) => Some(path),
        Err(e) if e.not_found() => None,
        Err(e) => {
            return Err(e).context("load dotenv");
        }
    };
    let args = Args::parse();
    setup_logging(args.logging_cfg).context("logging setup")?;
    info!(
        version = VERSION,
        revision = REVISION,
        dotenv_path = dotenv_path
            .as_ref()
            .map(|p| tracing::field::display(p.display())),
        "start common sense coder"
    );

    let mut tasks = JoinSet::new();

    let workspace = Arc::from(
        args.workspace
            .canonicalize()
            .context("canonicalize workspace path")?,
    );

    if let Some(intercept_io) = &args.intercept_io {
        tokio::fs::create_dir_all(intercept_io)
            .await
            .context("create directories for IO interception")?;
    }

    let quirks = args.programming_language.quirks();
    let (client, mut child) = spawn_lsp(
        &quirks,
        args.intercept_io.as_deref(),
        &args.workspace,
        &mut tasks,
    )
    .await
    .context("spawn LSP")?;
    let progress_guard = ProgressGuard::start(&mut tasks, &quirks, Arc::clone(&client));

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
        res = main_inner(quirks, client, progress_guard, workspace, stdin, stdout) => {
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
    quirks: Arc<dyn ProgrammingLanguageQuirks>,
    client: Arc<LspClient>,
    progress_guard: ProgressGuard,
    workspace: Arc<Path>,
    stdin: BoxRead,
    stdout: BoxWrite,
) -> Result<()> {
    let token_legend = init_lsp(&client, &workspace, &quirks)
        .await
        .context("init lsp")?;

    let service = CodeExplorer::new(progress_guard, token_legend, workspace)
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
