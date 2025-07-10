use std::path::Path;

use anyhow::{Context, Result};
use lsp_client::LspClient;
use lsp_types::{
    ClientCapabilities, ClientInfo, HoverClientCapabilities, InitializeParams, MarkupKind,
    SymbolKind, SymbolKindCapability, TextDocumentClientCapabilities,
    TextDocumentSyncClientCapabilities, WindowClientCapabilities, WorkspaceClientCapabilities,
    WorkspaceFolder, WorkspaceSymbolClientCapabilities,
};
use serde_json::json;
use tracing::info;

pub(crate) async fn init_lsp(client: &LspClient, workspace: &Path) -> Result<()> {
    info!("init LSP");

    client
        .initialize(InitializeParams {
            capabilities: ClientCapabilities {
                text_document: Some(TextDocumentClientCapabilities {
                    hover: Some(HoverClientCapabilities {
                        content_format: Some(vec![MarkupKind::Markdown]),
                        dynamic_registration: Some(false),
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
                name: env!("CARGO_PKG_NAME").to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            initialization_options: Some(json!({
                "files": {
                    "watcher": "server",
                },
                "hover": {
                    "dropGlue": {
                        "enable": false,
                    },
                    "memoryLayout": {
                        "enable": false,
                    },
                    "show": {
                        "enumVariants": 100,
                        "fields": 100,
                        "traitAssocItems": 100,
                    },
                },
                "workspace": {
                    "symbol": {
                        "search": {
                            "scope": "workspace_and_dependencies",
                        },
                    },
                },
            })),
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
