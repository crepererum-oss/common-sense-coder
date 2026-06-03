use std::{io::ErrorKind, ops::Deref, path::Path, sync::Arc};

use anyhow::Context;
use error::{OptionExt, ResultExt};
use lsp_client::LspClient;
use lsp_types::{
    DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams, HoverContents, HoverParams,
    LanguageString, Location, MarkedString, Range, ReferenceContext, ReferenceParams,
    SemanticTokensParams, SymbolInformation, SymbolKind, SymbolTag, TextDocumentIdentifier,
    TextDocumentPositionParams, WorkspaceSymbolParams, WorkspaceSymbolResponse,
    request::{
        DocumentSymbolRequest, GotoDeclaration, GotoDeclarationParams, GotoDefinition,
        GotoImplementation, GotoImplementationParams, GotoTypeDefinition, GotoTypeDefinitionParams,
        HoverRequest, References, SemanticTokensFullRequest,
    },
};
use rmcp::{
    Json, RoleServer, ServerHandler,
    handler::server::{
        tool::{ToolCallContext, ToolRouter},
        wrapper::Parameters,
    },
    model::{
        CallToolRequestParams, CallToolResult, ErrorData as McpError, Implementation,
        ListToolsResult, PaginatedRequestParams, ProgressNotificationParam, ServerCapabilities,
        ServerInfo,
    },
    schemars::{
        self, Schema,
        transform::{RestrictFormats, Transform},
    },
    service::RequestContext,
    tool, tool_router,
};
use search::SearchMode;
use tokio_stream::StreamExt;
use tracing::{debug, info};

use crate::{
    ProgressGuard,
    constants::{NAME, VERSION_STRING},
    lsp::{
        location::{LocationVariants, McpLocation, path_to_text_document_identifier, path_to_uri},
        progress_guard::Guard,
        requests::{
            WorkspaceSymbolParamsExt, WorkspaceSymbolRequestExt, WorkspaceSymbolScopeKindFiltering,
            WorkspaceSymbolSearchKind, WorkspaceSymbolSearchScope,
        },
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

    async fn wait_for_client(&self, ctx: RequestContext<RoleServer>) -> Guard<'_> {
        let fut_progress = async {
            if let Some(progress_token) = ctx.meta.get_progress_token() {
                let mut stream_evt = self.progress_guard.events();
                let mut progress = 0u32;

                while let Some(evt) = stream_evt.next().await {
                    ctx.peer
                        .notify_progress(ProgressNotificationParam {
                            progress_token: progress_token.clone(),
                            progress: progress as f64,
                            total: None,
                            message: Some(evt),
                        })
                        .await
                        .ok();
                    progress += 1;
                }
            }

            futures::future::pending::<()>().await
        };

        let fut_wait = async { self.progress_guard.wait().await };

        tokio::select! {
            _ = fut_progress => unreachable!(),
            guard = fut_wait => guard,
        }
    }

    async fn read_file(&self, file: &str) -> Result<Option<String>, McpError> {
        match tokio::fs::read_to_string(self.workspace.join(file)).await {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context("read file").internal(),
        }
    }

    fn filter_symbol_informations(
        &self,
        symbol_informations: &[SymbolInformation],
        query: Option<&str>,
        mode: SearchMode,
        workspace_and_dependencies: bool,
    ) -> Result<Vec<SymbolResult>, McpError> {
        let mut results = symbol_informations
            .iter()
            // rust-analyzer search is fuzzy by default
            .filter(|si| {
                query
                    .map(|query| mode.check(query, &si.name))
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
                    .as_ref()
                    .map(|tags| tags.contains(&SymbolTag::DEPRECATED))
                    .unwrap_or_default();

                let location = match McpLocation::try_new(
                    location.clone(),
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

                Ok(Some(SymbolResult {
                    name: name.to_owned(),
                    kind,
                    deprecated,
                    location,
                }))
            })
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>, _>>()?;

        results.sort_unstable();

        Ok(results)
    }

    async fn symbol_info_for_token(
        &self,
        token: &Token<'_>,
        path: &str,
        client: &LspClient,
        workspace_and_dependencies: bool,
    ) -> Result<Option<SymbolInfo>, McpError> {
        let location = token.mcp_location(path.to_owned(), Arc::clone(&self.workspace));

        let modifiers = token
            .token_modifiers()
            .iter()
            .map(|m| m.to_string())
            .collect::<Vec<_>>();

        let text_document_position_params = TextDocumentPositionParams::try_from(&location)
            .context("create text document position params")
            .internal()?;
        let Some(resp) = client
            .send_request::<HoverRequest>(HoverParams {
                text_document_position_params: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
            })
            .await
            .context("HoverRequest")
            .internal()?
        else {
            return Ok(None);
        };

        let hover = match resp.contents {
            HoverContents::Scalar(markup_string) => vec![HoverInfo::from(markup_string)],
            HoverContents::Array(marked_strings) => {
                marked_strings.into_iter().map(HoverInfo::from).collect()
            }
            HoverContents::Markup(markup_content) => {
                parse_markdown_code_blocks(&markup_content.value).unwrap_or_else(|| {
                    vec![HoverInfo {
                        language: None,
                        value: markup_content.value.trim().to_owned(),
                    }]
                })
            }
        };

        let declarations = match client
            .send_request::<GotoDeclaration>(GotoDeclarationParams {
                text_document_position_params: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .context("GotoDeclaration")
            .internal()?
        {
            Some(resp) => LocationVariants::from(resp)
                .into_mcp_location(Arc::clone(&self.workspace), workspace_and_dependencies)
                .context("convert declaration locations")
                .internal()?,
            None => vec![],
        };

        let definitions = match client
            .send_request::<GotoDefinition>(GotoDefinitionParams {
                text_document_position_params: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .context("GotoDefinition")
            .internal()?
        {
            Some(resp) => LocationVariants::from(resp)
                .into_mcp_location(Arc::clone(&self.workspace), workspace_and_dependencies)
                .context("convert definition locations")
                .internal()?,
            None => vec![],
        };

        let implementations = match client
            .send_request::<GotoImplementation>(GotoImplementationParams {
                text_document_position_params: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .context("GotoImplementation")
            .internal()?
        {
            Some(resp) => LocationVariants::from(resp)
                .into_mcp_location(Arc::clone(&self.workspace), workspace_and_dependencies)
                .context("convert implementation locations")
                .internal()?,
            None => vec![],
        };

        let type_definitions = match client
            .send_request::<GotoTypeDefinition>(GotoTypeDefinitionParams {
                text_document_position_params: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .context("GotoTypeDefinition")
            .internal()?
        {
            Some(resp) => LocationVariants::from(resp)
                .into_mcp_location(Arc::clone(&self.workspace), workspace_and_dependencies)
                .context("convert type definition locations")
                .internal()?,
            None => vec![],
        };

        let references = match client
            .send_request::<References>(ReferenceParams {
                text_document_position: text_document_position_params.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: ReferenceContext {
                    include_declaration: false,
                },
            })
            .await
            .context("References")
            .internal()?
        {
            Some(locations) => locations
                .into_iter()
                .map(|loc| {
                    McpLocation::try_new(
                        loc,
                        Arc::clone(&self.workspace),
                        workspace_and_dependencies,
                    )
                })
                .filter_map(Result::transpose)
                .collect::<Result<Vec<_>, _>>()
                .context("format references")
                .internal()?,
            None => vec![],
        };

        Ok(Some(SymbolInfo {
            token: TokenInfo {
                location,
                token_type: token.token_type().to_string(),
                modifiers,
            },
            hover,
            declarations,
            definitions,
            implementations,
            type_definitions,
            references,
        }))
    }
}

#[tool_router]
impl CodeExplorer {
    #[tool(
        description = "Find symbol (e.g. a struct, enum, method, ...) in code base. Use the `symbol_info` tool afterwards to learn more about the found symbols."
    )]
    async fn find_symbol(
        &self,
        Parameters(FindSymbolRequest {
            query,
            file,
            fuzzy,
            workspace_and_dependencies: workspace_and_dependencies_orig,
        }): Parameters<FindSymbolRequest>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<FindSymbolResult>, McpError> {
        let client = self.wait_for_client(ctx).await;

        let query = empty_string_to_none(query);
        let file = empty_string_to_none(file);
        let fuzzy = fuzzy.unwrap_or_default();
        let workspace_and_dependencies = workspace_and_dependencies_orig.unwrap_or_default();

        let symbol_informations = match file {
            Some(file) => {
                // LSP may error for non-existing files, so try to read it first
                let Some(file_content) = self.read_file(&file).await? else {
                    return Err(McpError::invalid_params(
                        format!("file not found: {file}"),
                        None,
                    ));
                };

                let resp = client
                    .send_request::<DocumentSymbolRequest>(DocumentSymbolParams {
                        text_document: TextDocumentIdentifier {
                            uri: path_to_uri(&self.workspace, &file)
                                .context("convert path to URI")
                                .internal()?,
                        },
                        work_done_progress_params: Default::default(),
                        partial_result_params: Default::default(),
                    })
                    .await
                    .context("DocumentSymbolRequest")
                    .internal()?;

                let mut symbol_informations = match resp {
                    None => {
                        // no symbols
                        vec![]
                    }
                    Some(DocumentSymbolResponse::Flat(symbol_informations)) => symbol_informations,
                    Some(DocumentSymbolResponse::Nested(_)) => {
                        return Err(McpError::internal_error(
                            "nested symbols are not yet implemented",
                            None,
                        ));
                    }
                };

                // variable declarations are not part of the symbol index, hence we need to fetch them manually
                let resp = client
                    .send_request::<SemanticTokensFullRequest>(SemanticTokensParams {
                        text_document: path_to_text_document_identifier(&self.workspace, &file)
                            .context("convert path to text document identifier")
                            .internal()?,
                        work_done_progress_params: Default::default(),
                        partial_result_params: Default::default(),
                    })
                    .await
                    .context("SemanticTokensFullRequest")
                    .internal()?;

                if let Some(lsp_types::SemanticTokensResult::Tokens(semantic_tokens)) = resp {
                    let doc = self
                        .token_legend
                        .decode(&file_content, semantic_tokens.data)
                        .context("decode semantic tokens")
                        .internal()?;

                    for token in doc.declared_variables() {
                        let location = Location {
                            uri: path_to_uri(&self.workspace, &file)
                                .context("convert path to URI")
                                .internal()?,
                            range: Range {
                                // in the then we just care about the position, so set both values to it
                                start: token.lsp_position(),
                                end: token.lsp_position(),
                            },
                        };

                        #[expect(deprecated, reason = "lsp-types still requires this field")]
                        let symbol_information = SymbolInformation {
                            name: token.data().to_owned(),
                            kind: SymbolKind::VARIABLE,
                            tags: token.is_deprecated().then_some(vec![SymbolTag::DEPRECATED]),
                            deprecated: None,
                            location,
                            container_name: None,
                        };
                        symbol_informations.push(symbol_information);
                    }
                }

                symbol_informations
            }
            None => {
                let query = query.as_ref().required("query".to_string())?;
                let resp = client
                    .send_request::<WorkspaceSymbolRequestExt>(WorkspaceSymbolParamsExt {
                        base: WorkspaceSymbolParams {
                            query: query.clone(),
                            ..Default::default()
                        },
                        filtering: WorkspaceSymbolScopeKindFiltering {
                            search_scope: Some(if workspace_and_dependencies {
                                WorkspaceSymbolSearchScope::WorkspaceAndDependencies
                            } else {
                                WorkspaceSymbolSearchScope::Workspace
                            }),
                            search_kind: Some(if workspace_and_dependencies {
                                // `WorkspaceSymbolSearchScope::WorkspaceAndDependencies` + `WorkspaceSymbolSearchKind::AllSymbols`
                                // SHOULD work with `AllSymbols` but seems to produce empty results. Maybe it's a bug
                                // in rust-analyzer or just not implemented. There are a some issues related to symbol
                                // filtering:
                                //
                                // - https://github.com/rust-lang/rust-analyzer/issues/13938
                                // - https://github.com/rust-lang/rust-analyzer/issues/16491
                                WorkspaceSymbolSearchKind::OnlyTypes
                            } else {
                                WorkspaceSymbolSearchKind::AllSymbols
                            }),
                        },
                    })
                    .await
                    .context("WorkspaceSymbolRequest")
                    .internal()?;

                let Some(resp) = resp else {
                    // no symbols
                    return Ok(Json(FindSymbolResult { symbols: vec![] }));
                };

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
        let mut results = self.filter_symbol_informations(
            &symbol_informations,
            query.as_deref(),
            mode,
            workspace_and_dependencies,
        )?;
        if results.is_empty() && workspace_and_dependencies_orig.is_none() {
            debug!("auto-expand scope to workspace_and_dependencies");
            results = self.filter_symbol_informations(
                &symbol_informations,
                query.as_deref(),
                mode,
                true,
            )?;
        }
        Ok(Json(FindSymbolResult { symbols: results }))
    }

    #[tool(
        description = "Get detailed information about a given symbol (struct, enum, method, trait, ...) like documentation, declaration, references, usage across the code base, etc."
    )]
    async fn symbol_info(
        &self,
        Parameters(SymbolInfoRequest {
            file,
            name,
            line,
            character,
            workspace_and_dependencies,
        }): Parameters<SymbolInfoRequest>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<SymbolInfoResult>, McpError> {
        let client = self.wait_for_client(ctx).await;

        let workspace_and_dependencies = workspace_and_dependencies.unwrap_or_default();

        let file_content = match self.read_file(&file).await? {
            Some(s) => s,
            None => {
                return Err(McpError::invalid_params(
                    format!("file not found: {file}"),
                    None,
                ));
            }
        };
        let resp = client
            .send_request::<SemanticTokensFullRequest>(SemanticTokensParams {
                text_document: path_to_text_document_identifier(&self.workspace, &file)
                    .context("convert path to text document identifier")
                    .internal()?,
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .context("SemanticTokensFullRequest")
            .internal()?
            .expected("language server did not provide any semantic tokens".to_owned())?;
        let doc = match resp {
            lsp_types::SemanticTokensResult::Tokens(semantic_tokens) => self
                .token_legend
                .decode(&file_content, semantic_tokens.data)
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
            let Some(res) = self
                .symbol_info_for_token(token, &file, &client, workspace_and_dependencies)
                .await?
            else {
                continue;
            };
            results.push(res);
        }

        Ok(Json(SymbolInfoResult { info: results }))
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct FindSymbolRequest {
    /// the symbol that you are looking for, required if `path` is not provided
    #[schemars(length(min = 1))]
    query: Option<String>,

    /// path to the file, otherwise search the entire workspace
    #[schemars(length(min = 1))]
    file: Option<String>,

    /// search fuzzy
    fuzzy: Option<bool>,

    /// search workspace and dependencies
    workspace_and_dependencies: Option<bool>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct FindSymbolResult {
    symbols: Vec<SymbolResult>,
}

#[derive(Debug, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
struct SymbolResult {
    name: String,
    kind: String,
    deprecated: bool,
    location: McpLocation,
}

impl PartialOrd for SymbolResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SymbolResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.location
            .cmp(&other.location)
            .then_with(|| self.name.cmp(&other.name))
            .then_with(|| self.kind.cmp(&other.kind))
    }
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct SymbolInfoResult {
    info: Vec<SymbolInfo>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct SymbolInfo {
    token: TokenInfo,
    hover: Vec<HoverInfo>,
    declarations: Vec<McpLocation>,
    definitions: Vec<McpLocation>,
    implementations: Vec<McpLocation>,
    type_definitions: Vec<McpLocation>,
    references: Vec<McpLocation>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct TokenInfo {
    location: McpLocation,
    token_type: String,
    modifiers: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SymbolInfoRequest {
    /// path to the file, can be absolute or relative
    file: String,

    /// symbol name
    name: String,

    /// 1-based line number within the file
    #[schemars(range(min = 1))]
    line: Option<u32>,

    /// 1-based character index within the line
    #[schemars(range(min = 1))]
    character: Option<u32>,

    /// search workspace and dependencies
    workspace_and_dependencies: Option<bool>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct HoverInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    value: String,
}

impl From<MarkedString> for HoverInfo {
    fn from(s: MarkedString) -> Self {
        match s {
            MarkedString::String(value) => parse_markdown_code_blocks(&value)
                .and_then(|mut blocks| (blocks.len() == 1).then(|| blocks.remove(0)))
                .unwrap_or_else(|| Self {
                    language: None,
                    value: value.trim().to_owned(),
                }),
            MarkedString::LanguageString(LanguageString { language, value }) => Self {
                language: Some(language),
                value,
            },
        }
    }
}

fn parse_markdown_code_blocks(value: &str) -> Option<Vec<HoverInfo>> {
    let mut rest = value.trim();
    let mut blocks = Vec::new();

    while !rest.is_empty() {
        let body = rest.strip_prefix("```")?;
        let (language, body) = body.split_once('\n')?;
        let (body, remaining) = body.split_once("```")?;
        blocks.push(HoverInfo {
            language: (!language.is_empty()).then(|| language.to_owned()),
            value: body.trim_end().to_owned(),
        });
        rest = remaining.trim();
    }

    (!blocks.is_empty()).then_some(blocks)
}

fn empty_string_to_none(s: Option<String>) -> Option<String> {
    s.and_then(|s| (!s.is_empty()).then_some(s))
}

impl ServerHandler for CodeExplorer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(NAME, VERSION_STRING))
            .with_instructions("\
                This server helps you to understand a code base.\
                \
                It comes with two tools:\
                - `find_symbols`: Searches symbols (structs, enums, methods, traits, ...) defined/used by the code base.\
                - `symbol_info`: Provides detailed information about a symbol like documentation and usage pattern.\
                \
                First use the `find_symbols` tool to get the file path of the respective symbol. Then use the `symbol_info` tool to get the detailed information about them.\
            ".trim().to_owned())
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        info!(name = request.name.as_ref(), "call tool");
        let tcc = ToolCallContext::new(self, request, context);
        self.tool_router.call(tcc).await
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let items = self.tool_router.list_all();

        // Workaround because some MCP users complain about non-standard formats.
        //
        // See <https://github.com/GREsau/schemars/pull/405>, but that's not used by [`rmcp`].
        let items = items
            .into_iter()
            .map(|mut tool| {
                let mut input_schema: Schema = tool.input_schema.deref().clone().into();
                RestrictFormats::default().transform(&mut input_schema);
                tool.input_schema = Arc::new(
                    input_schema
                        .as_object()
                        .expect("schema should be an object")
                        .clone(),
                );

                let mut output_schema: Schema = tool
                    .output_schema
                    .as_ref()
                    .expect("output schema set")
                    .deref()
                    .clone()
                    .into();
                RestrictFormats::default().transform(&mut output_schema);
                tool.output_schema = Some(Arc::new(
                    output_schema
                        .as_object()
                        .expect("schema should be an object")
                        .clone(),
                ));

                tool
            })
            .collect();

        Ok(ListToolsResult::with_all_items(items))
    }
}
