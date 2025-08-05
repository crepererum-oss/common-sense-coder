use lsp_types::request::{Request, WorkspaceSymbolRequest};
use serde::{Deserialize, Serialize};

/// Extended version of [`WorkspaceSymbolRequest`].
///
/// See <https://rust-analyzer.github.io/book/contributing/lsp-extensions.html#workspace-symbols-filtering>.
#[derive(Debug)]
pub(crate) enum WorkspaceSymbolRequestExt {}

impl Request for WorkspaceSymbolRequestExt {
    type Params = WorkspaceSymbolParamsExt;
    type Result = <WorkspaceSymbolRequest as Request>::Result;
    const METHOD: &'static str = <WorkspaceSymbolRequest as Request>::METHOD;
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSymbolParamsExt {
    #[serde(flatten)]
    pub(crate) base: <WorkspaceSymbolRequest as Request>::Params,

    #[serde(flatten)]
    pub(crate) filtering: WorkspaceSymbolScopeKindFiltering,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSymbolScopeKindFiltering {
    /// Return only the symbols defined in the specified scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) search_scope: Option<WorkspaceSymbolSearchScope>,

    /// Return only the symbols of specified kinds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) search_kind: Option<WorkspaceSymbolSearchKind>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum WorkspaceSymbolSearchScope {
    Workspace,
    WorkspaceAndDependencies,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum WorkspaceSymbolSearchKind {
    OnlyTypes,
    AllSymbols,
}
