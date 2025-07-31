use std::{
    path::{Path, PathBuf},
    process::{ExitCode, Termination},
    sync::Arc,
};

use anyhow::{Context, Result, ensure};
use clap::Parser;
use constants::{REVISION, VERSION, VERSION_STRING};
use futures::FutureExt;
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
use tasks::TaskManager;
use tracing::{debug, info, warn};

// used in integration tests
#[cfg(test)]
use assert_cmd as _;
#[cfg(test)]
use insta as _;
#[cfg(test)]
use nix as _;
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
mod tasks;

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

fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            // tokio and stdin may cause the process to hang
            // - https://github.com/tokio-rs/tokio/issues/2466
            // - https://github.com/chatmail/core/pull/4325/files
            let r = main_async().await;
            let exit_code = r.report();

            // manual implementation of `ExitCode::exit_process` because it's unstable,
            // see https://github.com/rust-lang/rust/issues/97100
            let exit_code = if exit_code == ExitCode::SUCCESS { 0 } else { 1 };
            std::process::exit(exit_code);
        })
}

async fn main_async() -> Result<()> {
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

    let mut tasks = TaskManager::new();

    let workspace = Arc::<Path>::from(
        args.workspace
            .canonicalize()
            .context("canonicalize workspace path")?,
    );
    info!(path=%workspace.display(), "workspace");

    if let Some(intercept_io) = &args.intercept_io {
        info!(path=%intercept_io.display(), "interception IO");

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
        res = main_inner(quirks, Arc::clone(&client), progress_guard, workspace, stdin, stdout) => {
            res.context("main")
        }
        e = tasks.run() => {
            Err(e).context("tasks")
        }
    };

    if let Err(e) = &res {
        warn!(%e, "system failed");
    }

    info!("shutdown server");

    debug!("dismantle LSP");
    res = res.and(
        async {
            client
                .shutdown()
                .await
                .context("shutdown language server")?;
            client.exit().await.context("exit language server")?;
            Ok(())
        }
        .await,
    );
    res = res.and(
        async {
            let status = child.wait().await.context("terminate language server")?;

            // `status.exit_ok` is unstable,
            // see https://github.com/rust-lang/rust/issues/84908
            ensure!(status.success(), "LSP exit was not clean: {status}");

            Ok(())
        }
        .await,
    );
    debug!("LSP gone");

    res = res.and(tasks.shutdown().await.context("task shutdown"));

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
    let ct = service.cancellation_token();
    let service_fut = service.waiting().fuse();
    let mut service_fut = std::pin::pin!(service_fut);

    let mut signal = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("create signal handler")?;

    tokio::select! {
        _ = signal.recv() => {
            info!("received shutdown signal");
            ct.cancel();
        }
        res = &mut service_fut => {
            res.context("wait for service")?;
        }
    }

    service_fut.await.context("wait for service")?;

    Ok(())
}
