use anyhow::{Context, Result};
use lsp_types::{SemanticToken, SemanticTokensLegend};

#[derive(Debug)]
pub(crate) struct TokenLegend {
    legend: SemanticTokensLegend,
}

impl TokenLegend {
    pub(crate) fn new(legend: SemanticTokensLegend) -> Self {
        Self { legend }
    }

    pub(crate) fn decode(&self, file: String, tokens: Vec<SemanticToken>) -> Result<Document> {
        let lines = file.lines().collect::<Vec<_>>();

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
                .legend
                .token_types
                .get(token_type as usize)
                .with_context(|| format!("invalid token type: {token_type}"))?
                .as_str()
                .to_owned();

            let range = (start as usize)..((start + length) as usize);
            let data = lines
                .get(line as usize)
                .with_context(|| format!("token line of of bounds: {line}"))?
                .get(range.clone())
                .with_context(|| format!("range out of bounds: {range:?}"))?
                .to_owned();

            doc_tokens.push(Token {
                line,
                start,
                token_type,
                data,
            })
        }

        Ok(Document { tokens: doc_tokens })
    }
}

#[derive(Debug)]
pub(crate) struct Document {
    tokens: Vec<Token>,
}

impl Document {
    pub(crate) fn query(
        &self,
        name: &str,
        line: Option<u32>,
        character: Option<u32>,
    ) -> Option<(u32, u32)> {
        self.tokens
            .iter()
            .filter(|token| token.data == name)
            .min_by_key(|token| {
                (
                    line.map(|line| line.abs_diff(token.line + 1)),
                    character.map(|character| character.abs_diff(token.start + 1)),
                )
            })
            .map(|token| (token.line + 1, token.start + 1))
    }
}

#[derive(Debug)]
struct Token {
    line: u32,
    start: u32,
    #[expect(dead_code)]
    token_type: String,
    data: String,
}
