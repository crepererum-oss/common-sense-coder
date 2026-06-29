use crate::setup::TestSetup;

#[tokio::test]
async fn test_list_tools() {
    let setup = TestSetup::new().await;

    insta::assert_json_snapshot!(
        setup.list_all_tools().await,
        @r##"
    [
      {
        "name": "find_symbol",
        "description": "Find symbol (e.g. a struct, enum, method, ...) in code base. Use the `symbol_info` tool afterwards to learn more about the found symbols.",
        "inputSchema": {
          "$schema": "https://json-schema.org/draft/2020-12/schema",
          "properties": {
            "query": {
              "description": "the symbol that you are looking for, required if `path` is not provided",
              "type": [
                "string",
                "null"
              ],
              "minLength": 1
            },
            "file": {
              "description": "path to the file, otherwise search the entire workspace",
              "type": [
                "string",
                "null"
              ],
              "minLength": 1
            },
            "fuzzy": {
              "description": "search fuzzy",
              "type": [
                "boolean",
                "null"
              ]
            },
            "workspace_and_dependencies": {
              "description": "search workspace and dependencies",
              "type": [
                "boolean",
                "null"
              ]
            }
          },
          "type": "object"
        },
        "outputSchema": {
          "$schema": "https://json-schema.org/draft/2020-12/schema",
          "$defs": {
            "SymbolResult": {
              "type": "object",
              "properties": {
                "name": {
                  "type": "string"
                },
                "kind": {
                  "type": "string"
                },
                "deprecated": {
                  "type": "boolean"
                },
                "location": {
                  "$ref": "#/$defs/Location"
                }
              },
              "required": [
                "name",
                "kind",
                "deprecated",
                "location"
              ]
            },
            "Location": {
              "description": "Describes a location of a symbol.",
              "type": "object",
              "properties": {
                "file": {
                  "description": "File path.",
                  "type": "string"
                },
                "line": {
                  "description": "1-based line number.",
                  "type": "integer",
                  "minimum": 1
                },
                "character": {
                  "description": "1-based character.",
                  "type": "integer",
                  "minimum": 1
                }
              },
              "required": [
                "file",
                "line",
                "character"
              ]
            }
          },
          "type": "object",
          "properties": {
            "symbols": {
              "type": "array",
              "items": {
                "$ref": "#/$defs/SymbolResult"
              }
            }
          },
          "required": [
            "symbols"
          ]
        }
      },
      {
        "name": "symbol_info",
        "description": "Get detailed information about a given symbol (struct, enum, method, trait, ...) like documentation, declaration, references, usage across the code base, etc.",
        "inputSchema": {
          "$schema": "https://json-schema.org/draft/2020-12/schema",
          "required": [
            "file",
            "name"
          ],
          "type": "object",
          "properties": {
            "file": {
              "description": "path to the file, can be absolute or relative",
              "type": "string"
            },
            "name": {
              "description": "symbol name",
              "type": "string"
            },
            "line": {
              "description": "1-based line number within the file",
              "type": [
                "integer",
                "null"
              ],
              "minimum": 1
            },
            "character": {
              "description": "1-based character index within the line",
              "type": [
                "integer",
                "null"
              ],
              "minimum": 1
            },
            "workspace_and_dependencies": {
              "description": "search workspace and dependencies",
              "type": [
                "boolean",
                "null"
              ]
            }
          }
        },
        "outputSchema": {
          "$schema": "https://json-schema.org/draft/2020-12/schema",
          "$defs": {
            "SymbolInfo": {
              "type": "object",
              "properties": {
                "token": {
                  "$ref": "#/$defs/TokenInfo"
                },
                "hover": {
                  "type": "array",
                  "items": {
                    "$ref": "#/$defs/HoverInfo"
                  }
                },
                "declarations": {
                  "type": "array",
                  "items": {
                    "$ref": "#/$defs/Location"
                  }
                },
                "definitions": {
                  "type": "array",
                  "items": {
                    "$ref": "#/$defs/Location"
                  }
                },
                "implementations": {
                  "type": "array",
                  "items": {
                    "$ref": "#/$defs/Location"
                  }
                },
                "type_definitions": {
                  "type": "array",
                  "items": {
                    "$ref": "#/$defs/Location"
                  }
                },
                "references": {
                  "type": "array",
                  "items": {
                    "$ref": "#/$defs/Location"
                  }
                }
              },
              "required": [
                "token",
                "hover",
                "declarations",
                "definitions",
                "implementations",
                "type_definitions",
                "references"
              ]
            },
            "TokenInfo": {
              "type": "object",
              "properties": {
                "location": {
                  "$ref": "#/$defs/Location"
                },
                "token_type": {
                  "type": "string"
                },
                "modifiers": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                }
              },
              "required": [
                "location",
                "token_type",
                "modifiers"
              ]
            },
            "Location": {
              "description": "Describes a location of a symbol.",
              "type": "object",
              "properties": {
                "file": {
                  "description": "File path.",
                  "type": "string"
                },
                "line": {
                  "description": "1-based line number.",
                  "type": "integer",
                  "minimum": 1
                },
                "character": {
                  "description": "1-based character.",
                  "type": "integer",
                  "minimum": 1
                }
              },
              "required": [
                "file",
                "line",
                "character"
              ]
            },
            "HoverInfo": {
              "type": "object",
              "properties": {
                "language": {
                  "type": [
                    "string",
                    "null"
                  ]
                },
                "value": {
                  "type": "string"
                }
              },
              "required": [
                "value"
              ]
            }
          },
          "type": "object",
          "properties": {
            "info": {
              "type": "array",
              "items": {
                "$ref": "#/$defs/SymbolInfo"
              }
            }
          },
          "required": [
            "info"
          ]
        }
      }
    ]
    "##,
    );

    setup.shutdown().await;
}
