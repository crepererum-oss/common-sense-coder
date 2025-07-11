use std::{path::Path, sync::Arc};

use error::{OptionExt, ResultExt};
use location::{LocationVariants, McpLocation, path_to_text_document_identifier, path_to_uri};
use lsp_types::{
    DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams, HoverContents, HoverParams,
    LanguageString, MarkedString, SymbolInformation, SymbolTag, TextDocumentIdentifier,
    TextDocumentPositionParams, WorkspaceSymbolParams, WorkspaceSymbolResponse,
    request::{DocumentSymbolRequest, GotoDefinition, HoverRequest, WorkspaceSymbolRequest},
};
use rmcp::{
    ServerHandler,
    handler::server::tool::{Parameters, ToolRouter},
    model::{CallToolResult, Content, ErrorData as McpError, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};
use search::SearchMode;
use tracing::debug;

use crate::ProgressGuard;

mod error;
mod location;
mod search;

#[derive(Debug)]
pub(crate) struct CodeExplorer {
    progress_guard: ProgressGuard,
    workspace: Arc<Path>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl CodeExplorer {
    pub(crate) fn new(progress_guard: ProgressGuard, workspace: Arc<Path>) -> Self {
        Self {
            progress_guard,
            workspace,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "find symbol (e.g. a struct, enum, method, ...) in code base")]
    async fn find_symbol(
        &self,
        Parameters(FindSymbolRequest {
            query,
            path,
            fuzzy,
            workspace_and_dependencies,
        }): Parameters<FindSymbolRequest>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.progress_guard.wait().await;

        let query = empty_string_to_none(query);
        let path = empty_string_to_none(path);

        let symbol_informations = match path {
            Some(path) => {
                let resp = client
                    .send_request::<DocumentSymbolRequest>(DocumentSymbolParams {
                        text_document: TextDocumentIdentifier {
                            uri: path_to_uri(&self.workspace, &path)?,
                        },
                        work_done_progress_params: Default::default(),
                        partial_result_params: Default::default(),
                    })
                    .await
                    .internal()?
                    .not_found(path)?;

                match resp {
                    DocumentSymbolResponse::Flat(symbol_informations) => symbol_informations,
                    DocumentSymbolResponse::Nested(_) => {
                        return Err(McpError::internal_error(
                            "nested symbols are not yet implemented",
                            None,
                        ));
                    }
                }
            }
            None => {
                let query = query.as_ref().required("query".to_string())?;
                let resp = client
                    .send_request::<WorkspaceSymbolRequest>(WorkspaceSymbolParams {
                        query: query.clone(),
                        ..Default::default()
                    })
                    .await
                    .internal()?
                    .not_found(query.clone())?;

                match resp {
                    WorkspaceSymbolResponse::Flat(symbol_informations) => symbol_informations,
                    WorkspaceSymbolResponse::Nested(_) => {
                        return Err(McpError::internal_error(
                            "nested symbols are not yet implemented",
                            None,
                        ));
                    }
                }
            }
        };

        let mode = if fuzzy {
            SearchMode::Fuzzy
        } else {
            SearchMode::Exact
        };
        let response = symbol_informations
            .into_iter()
            // rust-analyzer search is fuzzy by default
            .filter(|si| {
                query
                    .as_deref()
                    .map(|query| (mode.check(query, &si.name)))
                    .unwrap_or(true)
            })
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

                let McpLocation {
                    file,
                    line,
                    character,
                    workspace: _,
                } = match McpLocation::try_new(
                    location,
                    Arc::clone(&self.workspace),
                    workspace_and_dependencies,
                )? {
                    Some(loc) => loc,
                    None => {
                        return Ok(None);
                    }
                };

                let sr = SymbolResult {
                    name,
                    kind,
                    deprecated,
                    file,
                    line,
                    character,
                };
                let content = Content::json(sr)?;
                Ok(Some(content))
            })
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(CallToolResult::success(response))
    }

    #[tool(description = "get information to given symbol")]
    async fn symbol_info(
        &self,
        Parameters(SymbolInfoRequest {
            path,
            line,
            character,
            workspace_and_dependencies,
        }): Parameters<SymbolInfoRequest>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.progress_guard.wait().await;

        let character = match character {
            Some(c) => c,
            None => {
                // auto-detect character
                let resp = client
                    .send_request::<DocumentSymbolRequest>(DocumentSymbolParams {
                        text_document: path_to_text_document_identifier(&self.workspace, &path)?,
                        work_done_progress_params: Default::default(),
                        partial_result_params: Default::default(),
                    })
                    .await
                    .internal()?
                    .not_found(path.clone())?;

                let candidates = match resp {
                    DocumentSymbolResponse::Flat(symbol_informations) => symbol_informations
                        .into_iter()
                        .map(|si| si.location.range)
                        .filter(|range| range.start.line + 1 <= line && line <= range.end.line + 1)
                        .map(|range| range.start.character + 1)
                        .collect::<Vec<_>>(),
                    DocumentSymbolResponse::Nested(_) => {
                        return Err(McpError::internal_error(
                            "nested symbols are not yet implemented",
                            None,
                        ));
                    }
                };

                match candidates.as_slice() {
                    [] => {
                        return Err(McpError::resource_not_found(format!("{path}:{line}"), None));
                    }
                    [c] => {
                        debug!(path, line, character = *c, "auto-detected character");
                        *c
                    }
                    multiple => {
                        return Err(McpError::invalid_params(
                            format!("multiple symbols at {path}:{line} at position {multiple:?}"),
                            None,
                        ));
                    }
                }
            }
        };
        let location = McpLocation {
            file: path,
            line,
            character,
            workspace: self.workspace.clone(),
        };
        let text_document_position_params = TextDocumentPositionParams::try_from(&location)?;
        let resp = client
            .send_request::<HoverRequest>(HoverParams {
                text_document_position_params: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
            })
            .await
            .internal()?
            .not_found(location.to_string())?;

        let mut sections = match resp.contents {
            HoverContents::Scalar(markup_string) => {
                vec![format_marked_string(markup_string)]
            }
            HoverContents::Array(marked_strings) => marked_strings
                .into_iter()
                .map(format_marked_string)
                .collect(),
            HoverContents::Markup(markup_content) => vec![markup_content.value],
        };

        if let Some(resp) = client
            .send_request::<GotoDefinition>(GotoDefinitionParams {
                text_document_position_params,
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .internal()?
        {
            sections.push(format!(
                "Definition:\n\n{}",
                LocationVariants::from(resp)
                    .format(Arc::clone(&self.workspace), workspace_and_dependencies)?
            ))
        }

        Ok(CallToolResult::success(vec![Content::text(
            sections.join("\n\n---\n\n"),
        )]))
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct FindSymbolRequest {
    #[schemars(
        description = "the symbol that you are looking for, required if `path` is not provided",
        length(min = 1)
    )]
    query: Option<String>,

    #[schemars(
        description = "path to the file, otherwise search the entire workspace",
        default
    )]
    path: Option<String>,

    #[schemars(description = "search fuzzy", default)]
    fuzzy: bool,

    #[schemars(description = "search workspace and dependencies", default)]
    workspace_and_dependencies: bool,
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
    character: Option<u32>,

    #[schemars(description = "search workspace and dependencies", default)]
    workspace_and_dependencies: bool,
}

fn format_marked_string(s: MarkedString) -> String {
    match s {
        MarkedString::String(s) => s,
        MarkedString::LanguageString(LanguageString { language, value }) => {
            format!("```{language}\n{value}\n```\n")
        }
    }
}

fn empty_string_to_none(s: Option<String>) -> Option<String> {
    s.and_then(|s| (!s.is_empty()).then_some(s))
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
