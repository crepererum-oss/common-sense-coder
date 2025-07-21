use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use lsp_types::{SemanticToken, SemanticTokensLegend};

use super::location::McpLocation;

#[derive(Debug)]
pub(crate) struct TokenLegend {
    token_types: Vec<TokenType>,
}

impl TokenLegend {
    pub(crate) fn new(legend: SemanticTokensLegend) -> Self {
        Self {
            token_types: legend
                .token_types
                .into_iter()
                .map(|t| TokenType(t.as_str().to_owned()))
                .collect(),
        }
    }

    pub(crate) fn decode<'a>(
        &'a self,
        file_content: &'a str,
        tokens: Vec<SemanticToken>,
    ) -> Result<Document<'a>> {
        let lines = file_content.lines().collect::<Vec<_>>();

        let mut line = 0u32;
        let mut start = 0u32;
        let mut doc_tokens = Vec::with_capacity(tokens.len());

        for token in tokens {
            let SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type,
                token_modifiers_bitset: _,
            } = token;

            line += delta_line;
            start = if delta_line > 0 {
                delta_start
            } else {
                start + delta_start
            };

            let token_type = self
                .token_types
                .get(token_type as usize)
                .with_context(|| format!("invalid token type: {token_type}"))?;

            let range = (start as usize)..((start + length) as usize);
            let data = lines
                .get(line as usize)
                .with_context(|| format!("token line of of bounds: {line}"))?
                .get(range.clone())
                .with_context(|| format!("range out of bounds: {range:?}"))?;

            doc_tokens.push(Token {
                line: line + 1,
                character: start + 1,
                token_type,
                data,
            })
        }

        Ok(Document { tokens: doc_tokens })
    }
}

#[derive(Debug)]
pub(crate) struct Document<'legend> {
    tokens: Vec<Token<'legend>>,
}

impl<'legend> Document<'legend> {
    pub(crate) fn query(
        &self,
        name: &str,
        line: Option<u32>,
        character: Option<u32>,
    ) -> Option<&Token<'legend>> {
        self.tokens
            .iter()
            .filter(|token| token.data == name)
            .min_by_key(|token| {
                (
                    line.map(|line| line.abs_diff(token.line)),
                    character.map(|character| character.abs_diff(token.character)),
                )
            })
    }
}

#[derive(Debug)]
pub(crate) struct Token<'a> {
    /// 1-based line.
    line: u32,

    /// 1-based character.
    character: u32,

    /// Token type.
    token_type: &'a TokenType,

    /// Text data of the token.
    data: &'a str,
}

impl Token<'_> {
    pub(crate) fn location(&self, file: String, workspace: Arc<Path>) -> McpLocation {
        McpLocation {
            file,
            line: self.line,
            character: self.character,
            workspace,
        }
    }

    pub(crate) fn token_type(&self) -> &TokenType {
        self.token_type
    }
}

#[derive(Debug)]
pub(crate) struct TokenType(String);

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
