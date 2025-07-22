use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use anyhow::{Context, Error, Result};
use lsp_types::{
    GotoDefinitionResponse, Location, LocationLink, Position, TextDocumentIdentifier,
    TextDocumentPositionParams, Uri,
};

#[derive(Debug)]
pub(crate) enum LocationVariants {
    Scalar(Location),
    Array(Vec<Location>),
    Link(Vec<LocationLink>),
}

impl LocationVariants {
    pub(crate) fn format(
        self,
        workspace: Arc<Path>,
        workspace_and_dependencies: bool,
    ) -> Result<String> {
        Ok(match self {
            Self::Scalar(location) => {
                McpLocation::try_new(location, workspace, workspace_and_dependencies)?
                    .map(|loc| loc.to_string())
                    .unwrap_or_default()
            }
            Self::Array(locations) if locations.is_empty() => "None".to_owned(),
            Self::Array(locations) => {
                let locations = locations
                    .into_iter()
                    .map(|loc| {
                        McpLocation::try_new(
                            loc,
                            Arc::clone(&workspace),
                            workspace_and_dependencies,
                        )
                    })
                    .filter_map(Result::transpose)
                    .map(|res| res.map(|loc| format!("- {loc}")))
                    .collect::<Result<Vec<_>, _>>()
                    .context("format locations")?;
                locations.join("\n")
            }
            Self::Link(location_links) if location_links.is_empty() => "None".to_owned(),
            Self::Link(location_links) => {
                let locations = location_links
                    .into_iter()
                    .map(|loc| {
                        McpLocation::try_new_from_location_link(
                            loc,
                            Arc::clone(&workspace),
                            workspace_and_dependencies,
                        )
                    })
                    .filter_map(Result::transpose)
                    .map(|res| res.map(|loc| format!("- {loc}")))
                    .collect::<Result<Vec<_>, _>>()
                    .context("format locations")?;
                locations.join("\n")
            }
        })
    }
}

impl From<GotoDefinitionResponse> for LocationVariants {
    fn from(resp: GotoDefinitionResponse) -> Self {
        match resp {
            GotoDefinitionResponse::Scalar(location) => Self::Scalar(location),
            GotoDefinitionResponse::Array(locations) => Self::Array(locations),
            GotoDefinitionResponse::Link(location_links) => Self::Link(location_links),
        }
    }
}

#[derive(Debug)]
pub(crate) struct McpLocation {
    pub(crate) file: String,
    pub(crate) line: u32,
    pub(crate) character: u32,
    pub(crate) workspace: Arc<Path>,
}

impl McpLocation {
    pub(crate) fn try_new(
        loc: Location,
        workspace: Arc<Path>,
        workspace_and_dependencies: bool,
    ) -> Result<Option<Self>> {
        let Location { uri, range } = loc;

        let path = uri.path();
        let file = if path.is_absolute() {
            let path = PathBuf::from_str(path.as_str()).context("parse URI as path")?;

            // try to make it relative to the workspace root
            match (path.strip_prefix(&workspace), workspace_and_dependencies) {
                // path is within workspace
                (Ok(path2), _) => path2,
                // path outside workspace, but that's fine
                (Err(_), true) => &path,
                // path outside workspace, but we did not search for it
                (Err(_), false) => {
                    return Ok(None);
                }
            }
            .display()
            .to_string()
        } else {
            path.to_string()
        };

        let start = range.start;
        let line = start.line + 1;
        let character = start.character + 1;

        Ok(Some(Self {
            file,
            line,
            character,
            workspace,
        }))
    }

    pub(crate) fn try_new_from_location_link(
        loc: LocationLink,
        workspace: Arc<Path>,
        workspace_and_dependencies: bool,
    ) -> Result<Option<Self>> {
        let loc = Location::new(loc.target_uri, loc.target_range);
        Self::try_new(loc, workspace, workspace_and_dependencies)
    }
}

impl std::fmt::Display for McpLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            file,
            line,
            character,
            workspace: _,
        } = self;
        write!(f, "{file}:{line}:{character}")
    }
}

impl TryFrom<&McpLocation> for TextDocumentPositionParams {
    type Error = Error;

    fn try_from(loc: &McpLocation) -> Result<Self, Self::Error> {
        let McpLocation {
            file,
            line,
            character,
            workspace,
        } = loc;

        Ok(Self {
            text_document: path_to_text_document_identifier(workspace, file)?,
            position: Position {
                line: line - 1,
                character: character - 1,
            },
        })
    }
}

pub(crate) fn path_to_uri(workspace: &Path, path: &str) -> Result<Uri> {
    // prefix relative paths with workspace
    let path = if path.starts_with("/") {
        path
    } else {
        &format!("{}/{path}", workspace.display())
    };

    format!("file://{path}").parse().context("parse file URI")
}

pub(crate) fn path_to_text_document_identifier(
    workspace: &Path,
    path: &str,
) -> Result<TextDocumentIdentifier> {
    Ok(TextDocumentIdentifier {
        uri: path_to_uri(workspace, path)?,
    })
}
