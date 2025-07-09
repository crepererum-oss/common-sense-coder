use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use anyhow::Context;
use lsp_client::LspClient;
use lsp_types::{
    DocumentSymbolParams, DocumentSymbolResponse, SymbolInformation, SymbolTag,
    TextDocumentIdentifier, WorkspaceSymbolParams, WorkspaceSymbolResponse,
    request::{DocumentSymbolRequest, WorkspaceSymbolRequest},
};
use rmcp::{
    ServerHandler,
    handler::server::tool::{Parameters, ToolRouter},
    model::{
        Annotated, CallToolResult, Content, ErrorData as McpError, RawContent, ServerCapabilities,
        ServerInfo,
    },
    schemars, tool, tool_handler, tool_router,
};
use tracing::debug;

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

    #[tool(description = "list all symbols in a given file")]
    async fn file_symbols(
        &self,
        Parameters(FileSymbolRequest { path }): Parameters<FileSymbolRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.progress_guard.wait().await;

        let resp = self
            .client
            .send_request::<DocumentSymbolRequest>(DocumentSymbolParams {
                text_document: TextDocumentIdentifier {
                    uri: format!("file://{}/{path}", self.workspace.display())
                        .parse()
                        .internal()?,
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .internal()?
            .not_found(path)?;

        let response = match resp {
            DocumentSymbolResponse::Flat(symbol_informations) => {
                SymbolResult::si_vec_to_content(symbol_informations, &self.workspace)?
            }
            DocumentSymbolResponse::Nested(_) => {
                return Err(McpError::internal_error(
                    "nested symbols are not yet implemented",
                    None,
                ));
            }
        };

        Ok(CallToolResult::success(response))
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
            WorkspaceSymbolResponse::Flat(symbol_informations) => {
                SymbolResult::si_vec_to_content(symbol_informations, &self.workspace)?
            }
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
struct FileSymbolRequest {
    #[schemars(description = "path to the file")]
    path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct FindSymbolRequest {
    #[schemars(description = "the symbol that you are looking for")]
    query: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct SymbolResult {
    name: String,
    kind: String,
    deprecated: bool,
    file: String,
    line: u32,
}

impl SymbolResult {
    fn try_new(si: SymbolInformation, workspace: &Path) -> Result<Option<Self>, McpError> {
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
            let path = PathBuf::from_str(path.as_str())
                .context("parse URI as path")
                .internal()?;

            match path.strip_prefix(workspace) {
                Ok(path) => path.display().to_string(),
                Err(_) => {
                    debug!(path = %path.display(), "skip path outside workspace");
                    return Ok(None);
                }
            }
        } else {
            path.to_string()
        };

        let line = location.range.start.line + 1;

        Ok(Some(SymbolResult {
            name,
            kind,
            deprecated,
            file,
            line,
        }))
    }

    fn try_new_content(
        si: SymbolInformation,
        workspace: &Path,
    ) -> Result<Option<Annotated<RawContent>>, McpError> {
        match Self::try_new(si, workspace) {
            Ok(Some(sr)) => Ok(Some(Content::json(sr)?)),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn si_vec_to_content(
        symbol_informations: Vec<SymbolInformation>,
        workspace: &Path,
    ) -> Result<Vec<Annotated<RawContent>>, McpError> {
        symbol_informations
            .into_iter()
            .map(|si| Self::try_new_content(si, workspace))
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>, McpError>>()
    }
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
