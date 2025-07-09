use std::path::Path;

use anyhow::{Context, Result};
use lsp_client::LspClient;
use lsp_types::{
    ClientCapabilities, ClientInfo, InitializeParams, SymbolKind, SymbolKindCapability,
    WindowClientCapabilities, WorkspaceClientCapabilities, WorkspaceFolder,
    WorkspaceSymbolClientCapabilities,
};
use tracing::info;

pub(crate) async fn init_lsp(client: &LspClient, workspace: &Path) -> Result<()> {
    info!("init LSP");

    client
        .initialize(InitializeParams {
            capabilities: ClientCapabilities {
                workspace: Some(WorkspaceClientCapabilities {
                    workspace_folders: Some(true),
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
                    ..Default::default()
                }),
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
            client_info: Some(ClientInfo {
                name: env!("CARGO_PKG_NAME").to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
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
    client.initialized().await.context("set init response")?;

    info!("LSP initialized");

    Ok(())
}
