use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use itertools::Itertools;
use lsp_types::{SemanticToken, SemanticTokensLegend};

use super::location::McpLocation;

#[derive(Debug)]
pub(crate) struct TokenLegend {
    token_types: Vec<TokenType>,
    token_modifiers: Vec<TokenModifier>,
}

impl TokenLegend {
    pub(crate) fn new(legend: SemanticTokensLegend) -> Self {
        Self {
            token_types: legend
                .token_types
                .into_iter()
                .map(|t| TokenType(t.as_str().to_owned()))
                .collect(),
            token_modifiers: legend
                .token_modifiers
                .into_iter()
                .map(|t| TokenModifier(t.as_str().to_owned()))
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
                token_modifiers_bitset,
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
                token_modifiers: TokenModifers {
                    legend: &self.token_modifiers,
                    bitset: token_modifiers_bitset,
                },
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
    ) -> Vec<&Token<'legend>> {
        self.tokens
            .iter()
            .filter(|token| token.data == name)
            .min_set_by_key(|token| {
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

    /// Token modifiers.
    token_modifiers: TokenModifers<'a>,

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

    pub(crate) fn token_modifers(&self) -> TokenModifers<'_> {
        self.token_modifiers
    }
}

#[derive(Debug)]
pub(crate) struct TokenType(String);

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub(crate) struct TokenModifier(String);

impl std::fmt::Display for TokenModifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TokenModifers<'a> {
    legend: &'a [TokenModifier],
    bitset: u32,
}

impl TokenModifers<'_> {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &TokenModifier> {
        self.legend
            .iter()
            .zip(BitIter::new(self.bitset))
            .filter(|(_modifier, bit)| *bit)
            .map(|(modifier, _bit)| modifier)
    }
}

#[derive(Debug)]
struct BitIter {
    bitset_remaining: u32,
}

impl BitIter {
    fn new(bitset: u32) -> Self {
        Self {
            bitset_remaining: bitset,
        }
    }
}

impl Iterator for BitIter {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bitset_remaining == 0 {
            return None;
        }

        let found_bit = (self.bitset_remaining & 1) != 0;
        self.bitset_remaining >>= 1;
        Some(found_bit)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let h = u32::BITS - self.bitset_remaining.leading_zeros();
        let h = h as usize;
        (h, Some(h))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_bit_iter() {
        let mut it = BitIter::new(0);
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert_eq!(it.next(), None);

        let mut it = BitIter::new(1);
        assert_eq!(it.size_hint(), (1, Some(1)));
        assert_eq!(it.next(), Some(true));
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert_eq!(it.next(), None);

        let mut it = BitIter::new(2);
        assert_eq!(it.size_hint(), (2, Some(2)));
        assert_eq!(it.next(), Some(false));
        assert_eq!(it.size_hint(), (1, Some(1)));
        assert_eq!(it.next(), Some(true));
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert_eq!(it.next(), None);

        let mut it = BitIter::new(3);
        assert_eq!(it.size_hint(), (2, Some(2)));
        assert_eq!(it.next(), Some(true));
        assert_eq!(it.size_hint(), (1, Some(1)));
        assert_eq!(it.next(), Some(true));
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert_eq!(it.next(), None);

        let mut it = BitIter::new(4);
        assert_eq!(it.size_hint(), (3, Some(3)));
        assert_eq!(it.next(), Some(false));
        assert_eq!(it.size_hint(), (2, Some(2)));
        assert_eq!(it.next(), Some(false));
        assert_eq!(it.size_hint(), (1, Some(1)));
        assert_eq!(it.next(), Some(true));
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert_eq!(it.next(), None);

        let mut it = BitIter::new(5);
        assert_eq!(it.size_hint(), (3, Some(3)));
        assert_eq!(it.next(), Some(true));
        assert_eq!(it.size_hint(), (2, Some(2)));
        assert_eq!(it.next(), Some(false));
        assert_eq!(it.size_hint(), (1, Some(1)));
        assert_eq!(it.next(), Some(true));
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert_eq!(it.next(), None);
    }
}
