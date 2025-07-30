<p align="center">
  <a href="https://github.com/crepererum-oss/common-sense-coder">
    <img src="https://raw.githubusercontent.com/crepererum-oss/common-sense-coder/refs/heads/main/logo.svg" width=500 />
  </a>
</p>

# Common Sense Coder
An [LSP]-[MCP] bridge for code assistants.

## Usage
Compile the server:

```console
$ cargo build --release
```

The server can then be used as an `stdio` [MCP] server, e.g. for [Claude Code] you can add this to your `claude.json`:

```json
{
  "mcpServers": {
    "codeexplorer": {
      "type": "stdio",
      "command": "/the/location/of/common-sense-coder/target/release/common-sense-coder",
      "args": [
        "--workspace=."
      ],
      "env": {}
    }
  }
}
```

To see all arguments and possible environment variables, use:

```console
$ cargo run --release -- --help
```

## Background
This what-why-how is loosely inspired by [How to Write Better with The Why, What, How Framework, by Eugene Yan](https://eugeneyan.com/writing/writing-docs-why-what-how/#writing-framework-why-what-how-who).

### What
This allows an [LLM] like [Claude Code] to browse your codebase. This is done by using an existing [LSP] server like [rust-analyzer] and exposing an [LLM]-friendly interface via [MCP].

### Why
Watching [Claude Code] exploring my codebase was painful. It feels like a naive human that never used an [IDE] before. Sure an [LLM] can ready through a codebase faster than a human, but finding references and implementations – especially in dependencies – is hard without "go to definition" and "find all references".

There are a few [MCP]-[LSP] bridges like [LSP MCP] or [MCP Language Server]. However, they focus mostly on exposing the [LSP] mostly one-to-one as an [MCP]. It turns however that [LSP] is not super great for [LLM]s or humans. Listing symbols via [`textDocument/documentSymbol`] or [`workspace/symbol`] provides you with a symbol location as a character range of the entire "thing". So for a Rust function this would be this entire block:

```rust
/// A test method.
#[allow(some_lint)]
pub(crate) fn my_fn<T>(a: T, b: T) -> T where T: MyTrait {
    ...
}
```

To get the definition (via [`textDocument/definition`]), implementations (via [`textDocument/implementation`]), or references (via [`textDocument/references`]) you need the exact token location of the symbol, so in this case above `my_fn`. To get the token location, you either need to hope that the [LLM] guesses right or understands the somewhat complex [semantic tokens] encoding.

Either way, the [LLM] probably needs multiple very precise requests to get anything useful. For the [LSP] protocol that all makes sense, because it is optimized to be used with an [IDE]. While testing _common sense coder_ with [Claude Code], it turned out however that the [LLM] deals better with less technical, but more complete responses, even if they take longer to compute.

### How
The [MCP] implemented by _common sense coder_ basically provides two methods:

- **find things:** This allows the [LLM] to explore the codebase when it does not know the exact location of a method, trait, class, etc. yet.
- **details:** For a given symbol – which can be provided with slightly looser terms than the [LSP] wants to – the [LLM] can retrieve many details like documentation, signature, implementations, references, etc. in one go.

Under the hood that is implemented by carefully using the low level [LSP] methods and map between the more human-like [LLM] view and the [LSP] [semantic symbols].


## License

Licensed under either of these:

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
 * MIT License ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

### Contributing

Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in the work, as defined
in the Apache-2.0 license, shall be dual-licensed as above, without any additional terms or conditions.


[Claude Code]: https://www.anthropic.com/claude-code
[IDE]: https://en.wikipedia.org/wiki/Integrated_development_environment
[LLM]: https://en.wikipedia.org/wiki/Large_language_model
[LSP]: https://microsoft.github.io/language-server-protocol/
[LSP MCP]: https://github.com/jonrad/lsp-mcp
[MCP Language Server]: https://github.com/isaacphi/mcp-language-server
[MCP]: https://modelcontextprotocol.io/
[rust-analyzer]: https://rust-analyzer.github.io/
[semantic tokens]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_definition
[`textDocument/definition`]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_definition
[`textDocument/documentSymbol`]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_documentSymbol
[`textDocument/implementation`]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_definition
[`textDocument/references`]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_definition
[`workspace/symbol`]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#workspace_symbol
