use clap::ValueEnum;
use serde_json::json;
use std::{fmt::Debug, sync::Arc};

/// Code programming language.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ProgrammingLanguage {
    Rust,
}

impl ProgrammingLanguage {
    /// Get quirks for respective language.
    pub(crate) fn quirks(&self) -> Arc<dyn ProgrammingLanguageQuirks> {
        match self {
            Self::Rust => Arc::new(Rust),
        }
    }
}

/// Quirks for the respective [`ProgrammingLanguage`].
pub(crate) trait ProgrammingLanguageQuirks: Debug + Send + Sync + 'static {
    /// Binary name of the language server.
    fn language_server(&self) -> String;

    /// Language server initialization options.
    fn initialization_options(&self) -> Option<serde_json::Value>;
}

#[derive(Debug)]
struct Rust;

impl ProgrammingLanguageQuirks for Rust {
    fn language_server(&self) -> String {
        "rust-analyzer".to_owned()
    }

    fn initialization_options(&self) -> Option<serde_json::Value> {
        Some(json!({
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
        }))
    }
}
