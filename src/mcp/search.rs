//! Search mode implementation.
//!
//! Code borrowed from <https://github.com/rust-lang/rust-analyzer/blob/600f573256f7df1c4b2eb674577246d49561886f/crates/hir-def/src/import_map.rs#L290C1-L336C2>.

use rmcp::schemars;

/// How to search symbols.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default, serde::Deserialize, schemars::JsonSchema)]
pub(crate) enum SearchMode {
    /// Entry should strictly match the query string.
    #[default]
    Exact,
    /// Entry should contain all letters from the query string,
    /// in the same order, but not necessary adjacent.
    Fuzzy,
}

impl SearchMode {
    pub(crate) fn check(self, query: &str, candidate: &str) -> bool {
        match self {
            SearchMode::Exact => candidate == query,
            SearchMode::Fuzzy => {
                let mut name = candidate;
                query.chars().all(|query_char| {
                    let m = name.match_indices(query_char).next();
                    match m {
                        Some((index, _)) => {
                            name = name[index..]
                                .strip_prefix(|_: char| true)
                                .unwrap_or_default();
                            true
                        }
                        None => false,
                    }
                })
            }
        }
    }
}
