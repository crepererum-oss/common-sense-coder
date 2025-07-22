use std::{path::Path, sync::Arc};

use anyhow::Context;
use error::{OptionExt, ResultExt};
use itertools::Itertools;
use lsp_client::LspClient;
use lsp_types::{
    DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams, HoverContents, HoverParams,
    LanguageString, MarkedString, ReferenceContext, ReferenceParams, SemanticTokensParams,
    SymbolInformation, SymbolTag, TextDocumentIdentifier, TextDocumentPositionParams,
    WorkspaceSymbolParams, WorkspaceSymbolResponse,
    request::{
        DocumentSymbolRequest, GotoDeclaration, GotoDeclarationParams, GotoDefinition,
        GotoImplementation, GotoImplementationParams, GotoTypeDefinition, GotoTypeDefinitionParams,
        HoverRequest, References, SemanticTokensFullRequest, WorkspaceSymbolRequest,
    },
};
use rmcp::{
    ServerHandler,
    handler::server::tool::{Parameters, ToolRouter},
    model::{
        CallToolResult, Content, ErrorData as McpError, Implementation, ServerCapabilities,
        ServerInfo,
    },
    schemars, tool, tool_handler, tool_router,
};
use search::SearchMode;

use crate::{
    ProgressGuard,
    constants::{NAME, VERSION_STRING},
    lsp::{
        location::{LocationVariants, McpLocation, path_to_text_document_identifier, path_to_uri},
        tokens::{Token, TokenLegend},
    },
};

mod error;
mod search;

#[derive(Debug)]
pub(crate) struct CodeExplorer {
    progress_guard: ProgressGuard,
    token_legend: TokenLegend,
    workspace: Arc<Path>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl CodeExplorer {
    pub(crate) fn new(
        progress_guard: ProgressGuard,
        token_legend: TokenLegend,
        workspace: Arc<Path>,
    ) -> Self {
        Self {
            progress_guard,
            token_legend,
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
        let fuzzy = fuzzy.unwrap_or_default();
        let workspace_and_dependencies = workspace_and_dependencies.unwrap_or_default();

        let symbol_informations = match path {
            Some(path) => {
                let resp = client
                    .send_request::<DocumentSymbolRequest>(DocumentSymbolParams {
                        text_document: TextDocumentIdentifier {
                            uri: path_to_uri(&self.workspace, &path)
                                .context("convert path to URI")
                                .internal()?,
                        },
                        work_done_progress_params: Default::default(),
                        partial_result_params: Default::default(),
                    })
                    .await
                    .context("DocumentSymbolRequest")
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
                    .context("WorkspaceSymbolRequest")
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

                let deprecated = tags.unwrap_or_default().contains(&SymbolTag::DEPRECATED);

                let McpLocation {
                    file,
                    line,
                    character,
                    workspace: _,
                } = match McpLocation::try_new(
                    location,
                    Arc::clone(&self.workspace),
                    workspace_and_dependencies,
                )
                .context("create MCP location")
                .internal()?
                {
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
            name,
            line,
            character,
            workspace_and_dependencies,
        }): Parameters<SymbolInfoRequest>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.progress_guard.wait().await;

        let workspace_and_dependencies = workspace_and_dependencies.unwrap_or_default();

        let resp = client
            .send_request::<SemanticTokensFullRequest>(SemanticTokensParams {
                text_document: path_to_text_document_identifier(&self.workspace, &path)
                    .context("convert path to text document identifier")
                    .internal()?,
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .context("SemanticTokensFullRequest")
            .internal()?
            .not_found(path.clone())?;
        let file = tokio::fs::read_to_string(self.workspace.join(&path))
            .await
            .context("read file")
            .internal()?;
        let doc = match resp {
            lsp_types::SemanticTokensResult::Tokens(semantic_tokens) => self
                .token_legend
                .decode(&file, semantic_tokens.data)
                .context("decode semantic tokens")
                .internal()?,
            lsp_types::SemanticTokensResult::Partial(_) => {
                return Err(McpError::internal_error(
                    "partial semantic token results are not supported",
                    None,
                ));
            }
        };
        let tokens = doc.query(&name, line, character);
        let mut results = vec![];
        for token in tokens {
            results.push(Content::text(
                self.symbol_info_for_token(token, &path, &client, workspace_and_dependencies)
                    .await?,
            ));
        }

        Ok(CallToolResult::success(results))
    }

    async fn symbol_info_for_token(
        &self,
        token: &Token<'_>,
        path: &str,
        client: &LspClient,
        workspace_and_dependencies: bool,
    ) -> Result<String, McpError> {
        let location = token.location(path.to_owned(), Arc::clone(&self.workspace));

        let mut sections = vec![format!(
            "Token:\n\n- location: {location}\n- type: {}\n- modifiers: {}",
            token.token_type(),
            token
                .token_modifers()
                .iter()
                .map(|m| m.to_string())
                .join(", "),
        )];

        let text_document_position_params = TextDocumentPositionParams::try_from(&location)
            .context("create text document position params")
            .internal()?;
        let resp = client
            .send_request::<HoverRequest>(HoverParams {
                text_document_position_params: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
            })
            .await
            .internal()?
            .not_found(location.to_string())?;

        sections.extend(match resp.contents {
            HoverContents::Scalar(markup_string) => {
                vec![format_marked_string(markup_string)]
            }
            HoverContents::Array(marked_strings) => marked_strings
                .into_iter()
                .map(format_marked_string)
                .collect(),
            HoverContents::Markup(markup_content) => vec![markup_content.value.trim().to_owned()],
        });

        if let Some(resp) = client
            .send_request::<GotoDeclaration>(GotoDeclarationParams {
                text_document_position_params: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .internal()?
        {
            sections.push(format!(
                "Declaration:\n{}",
                LocationVariants::from(resp)
                    .format(Arc::clone(&self.workspace), workspace_and_dependencies)
                    .context("format location variants")
                    .internal()?
            ))
        }

        if let Some(resp) = client
            .send_request::<GotoDefinition>(GotoDefinitionParams {
                text_document_position_params: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .internal()?
        {
            sections.push(format!(
                "Definition:\n{}",
                LocationVariants::from(resp)
                    .format(Arc::clone(&self.workspace), workspace_and_dependencies)
                    .context("format location variants")
                    .internal()?
            ))
        }

        if let Some(resp) = client
            .send_request::<GotoImplementation>(GotoImplementationParams {
                text_document_position_params: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .internal()?
        {
            sections.push(format!(
                "Implementation:\n{}",
                LocationVariants::from(resp)
                    .format(Arc::clone(&self.workspace), workspace_and_dependencies)
                    .context("format location variants")
                    .internal()?
            ))
        }

        if let Some(resp) = client
            .send_request::<GotoTypeDefinition>(GotoTypeDefinitionParams {
                text_document_position_params: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .internal()?
        {
            sections.push(format!(
                "Type Definition:\n{}",
                LocationVariants::from(resp)
                    .format(Arc::clone(&self.workspace), workspace_and_dependencies)
                    .context("format location variants")
                    .internal()?
            ))
        }

        if let Some(locations) = client
            .send_request::<References>(ReferenceParams {
                text_document_position: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: ReferenceContext {
                    include_declaration: false,
                },
            })
            .await
            .internal()?
        {
            let locations = locations
                .into_iter()
                .filter_map(|loc| {
                    McpLocation::try_new(
                        loc,
                        Arc::clone(&self.workspace),
                        workspace_and_dependencies,
                    )
                    .ok()
                    .flatten()
                })
                .map(|loc| format!("- {loc}"))
                .collect::<Vec<_>>();
            let locations = if locations.is_empty() {
                "None".to_owned()
            } else {
                locations.join("\n")
            };
            sections.push(format!("References:\n{locations}"));
        }

        Ok(sections.join("\n\n---\n\n"))
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

    #[schemars(description = "search fuzzy")]
    fuzzy: Option<bool>,

    #[schemars(description = "search workspace and dependencies")]
    workspace_and_dependencies: Option<bool>,
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

    #[schemars(description = "symbol name")]
    name: String,

    #[schemars(description = "1-based line number within the file", range(min = 1))]
    line: Option<u32>,

    #[schemars(
        description = "1-based character index within the line",
        range(min = 1)
    )]
    character: Option<u32>,

    #[schemars(description = "search workspace and dependencies", default)]
    workspace_and_dependencies: Option<bool>,
}

fn format_marked_string(s: MarkedString) -> String {
    match s {
        MarkedString::String(s) => s.trim().to_owned(),
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
            protocol_version: Default::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: NAME.to_owned(),
                version: VERSION_STRING.to_owned(),
            },
            instructions: Some("A code exporer".into()),
        }
    }
}
