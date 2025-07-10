use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use anyhow::Context;
use error::{OptionExt, ResultExt};
use lsp_client::LspClient;
use lsp_types::{
    DocumentSymbolParams, DocumentSymbolResponse, HoverContents, HoverParams, LanguageString,
    MarkedString, Position, SymbolInformation, SymbolTag, TextDocumentIdentifier,
    TextDocumentPositionParams, Uri, WorkspaceSymbolParams, WorkspaceSymbolResponse,
    request::{DocumentSymbolRequest, HoverRequest, WorkspaceSymbolRequest},
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

use crate::ProgressGuard;

mod error;

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

    fn path_to_uri(&self, path: &str) -> Result<Uri, McpError> {
        // prefix relative paths with workspace
        let path = if path.starts_with("/") {
            path
        } else {
            &format!("{}/{path}", self.workspace.display())
        };

        format!("file://{path}").parse().internal()
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
                    uri: self.path_to_uri(&path)?,
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
            .not_found(query.clone())?;

        let response = match resp {
            WorkspaceSymbolResponse::Flat(symbol_informations) => SymbolResult::si_vec_to_content(
                symbol_informations
                    .into_iter()
                    // rust-analyzer search is fuzzy and rather unhelpful, so bring it back to something sane
                    // TODO: make this a parameter
                    .filter(|si| si.name == query)
                    .collect(),
                &self.workspace,
            )?,
            WorkspaceSymbolResponse::Nested(_) => {
                return Err(McpError::internal_error(
                    "nested symbols are not yet implemented",
                    None,
                ));
            }
        };

        Ok(CallToolResult::success(response))
    }

    #[tool(description = "get information to given symbol")]
    async fn symbol_info(
        &self,
        Parameters(SymbolInfoRequest {
            path,
            line,
            character,
        }): Parameters<SymbolInfoRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.progress_guard.wait().await;

        let uri = self.path_to_uri(&path)?;

        let resp = self
            .client
            .send_request::<HoverRequest>(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position: Position {
                        line: line - 1,
                        character: character - 1,
                    },
                },
                work_done_progress_params: Default::default(),
            })
            .await
            .internal()?
            .not_found(format!("{path}:{line}:{character}"))?;

        let res = match resp.contents {
            HoverContents::Scalar(markup_string) => {
                vec![Content::text(format_marked_string(markup_string))]
            }
            HoverContents::Array(marked_strings) => marked_strings
                .into_iter()
                .map(format_marked_string)
                .map(Content::text)
                .collect(),
            HoverContents::Markup(markup_content) => vec![Content::text(markup_content.value)],
        };

        Ok(CallToolResult::success(res))
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
    character: u32,
}

impl SymbolResult {
    fn try_new(si: SymbolInformation, workspace: &Path) -> Result<Self, McpError> {
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

            // try to make it relative to the workspace root
            path.strip_prefix(workspace)
                .unwrap_or(&path)
                .display()
                .to_string()
        } else {
            path.to_string()
        };

        let start = location.range.start;
        let line = start.line + 1;
        let character = start.character + 1;

        Ok(SymbolResult {
            name,
            kind,
            deprecated,
            file,
            line,
            character,
        })
    }

    fn si_vec_to_content(
        symbol_informations: Vec<SymbolInformation>,
        workspace: &Path,
    ) -> Result<Vec<Annotated<RawContent>>, McpError> {
        symbol_informations
            .into_iter()
            .map(|si| Self::try_new(si, workspace).and_then(Content::json))
            .collect::<Result<Vec<_>, McpError>>()
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SymbolInfoRequest {
    #[schemars(description = "path to the file")]
    path: String,

    #[schemars(description = "1-based line number within the file", range(min = 1))]
    line: u32,

    #[schemars(
        description = "1-based character index within the line",
        range(min = 1)
    )]
    character: u32,
}

fn format_marked_string(s: MarkedString) -> String {
    match s {
        MarkedString::String(s) => s,
        MarkedString::LanguageString(LanguageString { language, value }) => {
            format!("```{language}\n{value}\n```\n")
        }
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
