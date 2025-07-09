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
                            value_set: Some(vec![
                                SymbolKind::CLASS,
                                SymbolKind::FUNCTION,
                                SymbolKind::STRUCT,
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
