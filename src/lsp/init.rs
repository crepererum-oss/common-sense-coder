use std::{path::Path, process::Stdio, sync::Arc};

use anyhow::{Context, Result, bail, ensure};
use lsp_client::{LspClient, transport::io_transport};
use lsp_types::{
    ClientCapabilities, ClientInfo, GeneralClientCapabilities, HoverClientCapabilities,
    InitializeParams, MarkupKind, PositionEncodingKind, SemanticTokensClientCapabilities,
    SemanticTokensClientCapabilitiesRequests, SemanticTokensFullOptions,
    SemanticTokensServerCapabilities, SymbolKind, SymbolKindCapability,
    TextDocumentClientCapabilities, TextDocumentSyncClientCapabilities, WindowClientCapabilities,
    WorkspaceClientCapabilities, WorkspaceFolder, WorkspaceSymbolClientCapabilities,
};
use tokio::{
    process::{Child, Command},
    task::JoinSet,
};
use tracing::{debug, info};

use crate::{
    constants::{NAME, VERSION_STRING},
    io_intercept::{BoxRead, BoxWrite, ReadFork, WriteFork},
    lang::ProgrammingLanguageQuirks,
};

use super::tokens::TokenLegend;

pub(crate) async fn spawn_lsp(
    quirks: &Arc<dyn ProgrammingLanguageQuirks>,
    intercept_io: Option<&Path>,
    workspace: &Path,
    tasks: &mut JoinSet<Result<()>>,
) -> Result<(Arc<LspClient>, Child)> {
    let stderr = if let Some(intercept_io) = intercept_io {
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

    let mut child = Command::new(quirks.language_server())
        .current_dir(workspace)
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .context("cannot spawn language server")?;

    let stdin = Box::pin(child.stdin.take().expect("just initialized")) as BoxWrite;
    let stdout = Box::pin(child.stdout.take().expect("just initialized")) as BoxRead;
    let (stdin, stdout) = if let Some(intercept_io) = intercept_io {
        let stdin =
            Box::pin(WriteFork::new(stdin, intercept_io, "lsp.stdin.txt", tasks).await?) as _;
        let stdout =
            Box::pin(ReadFork::new(stdout, intercept_io, "lsp.stdout.txt", tasks).await?) as _;
        (stdin, stdout)
    } else {
        (stdin, stdout)
    };
    let (tx, rx) = io_transport(stdin, stdout);
    let client = Arc::new(LspClient::new(tx, rx));
    Ok((client, child))
}

pub(crate) async fn init_lsp(
    client: &LspClient,
    workspace: &Path,
    quirks: &Arc<dyn ProgrammingLanguageQuirks>,
) -> Result<TokenLegend> {
    debug!("initializing LSP");

    let init_results = client
        .initialize(InitializeParams {
            capabilities: ClientCapabilities {
                general: Some(GeneralClientCapabilities {
                    position_encodings: Some(vec![PositionEncodingKind::UTF8]),
                    ..Default::default()
                }),
                text_document: Some(TextDocumentClientCapabilities {
                    hover: Some(HoverClientCapabilities {
                        content_format: Some(vec![MarkupKind::Markdown]),
                        dynamic_registration: Some(false),
                    }),
                    semantic_tokens: Some(SemanticTokensClientCapabilities {
                        dynamic_registration: Some(false),
                        multiline_token_support: Some(false),
                        overlapping_token_support: Some(false),
                        requests: SemanticTokensClientCapabilitiesRequests {
                            range: Some(false),
                            full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
                        },
                        ..Default::default()
                    }),
                    synchronization: Some(TextDocumentSyncClientCapabilities {
                        did_save: Some(false),
                        dynamic_registration: Some(false),
                        will_save: Some(false),
                        will_save_wait_until: Some(false),
                    }),
                    ..Default::default()
                }),
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..Default::default()
                }),
                workspace: Some(WorkspaceClientCapabilities {
                    symbol: Some(WorkspaceSymbolClientCapabilities {
                        symbol_kind: Some(SymbolKindCapability {
                            // roughly based on
                            // https://github.com/rust-lang/rust-analyzer/blob/e429bac8793c24a99b643c4813ece813901c8c79/crates/rust-analyzer/src/lsp/to_proto.rs#L125-L179
                            value_set: Some(vec![
                                SymbolKind::CONSTANT,
                                SymbolKind::ENUM,
                                SymbolKind::ENUM_MEMBER,
                                SymbolKind::FIELD,
                                SymbolKind::FUNCTION,
                                SymbolKind::INTERFACE,
                                SymbolKind::METHOD,
                                SymbolKind::MODULE,
                                SymbolKind::NAMESPACE,
                                SymbolKind::OBJECT,
                                SymbolKind::STRUCT,
                                SymbolKind::TYPE_PARAMETER,
                                SymbolKind::VARIABLE,
                            ]),
                        }),
                        ..Default::default()
                    }),
                    workspace_folders: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
            client_info: Some(ClientInfo {
                name: NAME.to_owned(),
                version: Some(VERSION_STRING.to_owned()),
            }),
            initialization_options: quirks.initialization_options(),
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: format!("file://{}", workspace.display())
                    .parse()
                    .context("cannot parse workspace URI")?,
                name: "root".to_owned(),
            }]),
            ..Default::default()
        })
        .await
        .context("initialize language server")?;

    let server_caps = init_results.capabilities;

    ensure!(
        server_caps
            .position_encoding
            .context("language server reports position encoding")?
            == PositionEncodingKind::UTF8,
        "position encoding is UTF-8"
    );

    let token_legend = match server_caps
        .semantic_tokens_provider
        .context("expect language server to support semantic tokens")?
    {
        SemanticTokensServerCapabilities::SemanticTokensOptions(semantic_tokens_options) => {
            // check encoding
            let full = semantic_tokens_options
                .full
                .context("language server supports semantic tokens for full document")?;
            let uses_delta = match full {
                lsp_types::SemanticTokensFullOptions::Bool(_) => false,
                lsp_types::SemanticTokensFullOptions::Delta { delta } => delta.unwrap_or_default(),
            };
            ensure!(
                uses_delta,
                "language server uses delta mode to transfer semantic tokens"
            );

            // set up legend
            TokenLegend::new(semantic_tokens_options.legend, quirks)
        }
        SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(_) => {
            bail!("dynamic token registration not supported");
        }
    };

    client.initialized().await.context("set init response")?;

    let server_info = init_results.server_info;
    info!(
        server_name = server_info.as_ref().map(|info| info.name.as_str()),
        server_version = server_info
            .as_ref()
            .and_then(|info| info.version.as_deref()),
        "LSP initialized"
    );

    Ok(token_legend)
}
