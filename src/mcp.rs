use std::{path::PathBuf, str::FromStr, sync::Arc};

use anyhow::Context;
use lsp_client::LspClient;
use lsp_types::{
    SymbolInformation, SymbolTag, WorkspaceSymbolParams, WorkspaceSymbolResponse,
    request::WorkspaceSymbolRequest,
};
use rmcp::{
    ServerHandler,
    handler::server::tool::{Parameters, ToolRouter},
    model::{CallToolResult, Content, ErrorData as McpError, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};

use crate::ProgressGuard;

#[derive(Debug)]
pub(crate) struct CodeExplorer {
    client: Arc<LspClient>,
    progress_guard: ProgressGuard,
    workspace: PathBuf,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl CodeExplorer {
    pub(crate) fn new(
        client: Arc<LspClient>,
        progress_guard: ProgressGuard,
        workspace: PathBuf,
    ) -> Self {
        Self {
            client,
            progress_guard,
            workspace,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "find symbol in code base")]
    async fn find_symbol(
        &self,
        Parameters(FindSymbolRequest { query }): Parameters<FindSymbolRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.progress_guard.wait().await;

        let resp = self
            .client
            .send_request::<WorkspaceSymbolRequest>(WorkspaceSymbolParams {
                query: query.clone(),
                ..Default::default()
            })
            .await
            .internal()?
            .not_found(query)?;

        let response = match resp {
            WorkspaceSymbolResponse::Flat(symbol_informations) => symbol_informations
                .into_iter()
                .map(|si| {
                    let SymbolInformation {
                        name,
                        kind,
                        tags,
                        location,
                        ..
                    } = si;

                    let kind = format!("{kind:?}");

                    let deprecated = tags
                        .unwrap_or_default()
                        .iter()
                        .any(|tag| *tag == SymbolTag::DEPRECATED);

                    let path = location.uri.path();
                    let file = if path.is_absolute() {
                        PathBuf::from_str(path.as_str())
                            .context("parse URI as path")
                            .internal()?
                            .strip_prefix(&self.workspace)
                            .context("make path relative")
                            .internal()?
                            .display()
                            .to_string()
                    } else {
                        path.to_string()
                    };

                    let line = location.range.start.line + 1;

                    let res = FindSymbolResult {
                        name,
                        kind,
                        deprecated,
                        file,
                        line,
                    };
                    Content::json(res)
                })
                .collect::<Result<Vec<_>, McpError>>()?,
            WorkspaceSymbolResponse::Nested(_) => {
                return Err(McpError::internal_error(
                    "nested symbols are not yet implemented",
                    None,
                ));
            }
        };

        Ok(CallToolResult::success(response))
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct FindSymbolRequest {
    #[schemars(description = "the symbol that you are looking for")]
    query: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct FindSymbolResult {
    name: String,
    kind: String,
    deprecated: bool,
    file: String,
    line: u32,
}

#[tool_handler]
impl ServerHandler for CodeExplorer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("A code exporer".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

trait ErrorExt {
    fn internal(self) -> McpError;
}

impl<E> ErrorExt for E
where
    E: std::fmt::Display,
{
    fn internal(self) -> McpError {
        McpError::internal_error(self.to_string(), None)
    }
}

trait ResultExt {
    type T;

    fn internal(self) -> Result<Self::T, McpError>;
}

impl<T, E> ResultExt for Result<T, E>
where
    E: std::fmt::Display,
{
    type T = T;

    fn internal(self) -> Result<Self::T, McpError> {
        self.map_err(|e| e.internal())
    }
}

trait OptionExt {
    type T;

    fn not_found(self, what: String) -> Result<Self::T, McpError>;
}

impl<T> OptionExt for Option<T> {
    type T = T;

    fn not_found(self, what: String) -> Result<Self::T, McpError> {
        self.ok_or_else(|| McpError::resource_not_found(what, None))
    }
}
