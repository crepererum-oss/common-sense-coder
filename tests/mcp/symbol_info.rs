use serde_json::json;

use crate::setup::{TestSetup, map};

#[tokio::test]
async fn test_info_for_all_in_file() {
    let setup = TestSetup::new().await;

    let file = "src/lib.rs";

    let symbols = setup.find_symbol_ok(map([("file", json!(file))])).await;
    let symbols = symbols["symbols"].as_array().expect("symbols array");

    let mut results = Vec::new();
    for symbol in symbols {
        let name = symbol["name"].as_str().expect("str").to_owned();
        let location = &symbol["location"];
        let line = location["line"].as_u64().expect("u64");
        let character = location["character"].as_u64().expect("u64");

        let response = setup
            .symbol_info_ok(map([
                ("file", json!(file)),
                ("name", json!(name)),
                ("line", json!(line)),
                ("character", json!(character)),
            ]))
            .await;

        results.push(json!({
            "inputs": {
                "file": file,
                "name": name,
                "line": line,
                "character": character,
            },
            "response": response,
        }));
    }

    insta::assert_json_snapshot!(results, @r#"
    [
      {
        "inputs": {
          "file": "src/lib.rs",
          "name": "sub",
          "line": 5,
          "character": 1
        },
        "response": {
          "info": [
            {
              "token": {
                "location": {
                  "file": "src/lib.rs",
                  "line": 5,
                  "character": 5
                },
                "token_type": "namespace",
                "modifiers": [
                  "declaration"
                ]
              },
              "hover": [
                {
                  "language": "rust",
                  "value": "main_lib"
                },
                {
                  "language": "rust",
                  "value": "mod sub"
                }
              ],
              "declarations": [
                {
                  "file": "src/lib.rs",
                  "line": 5,
                  "character": 5
                }
              ],
              "definitions": [
                {
                  "file": "src/sub.rs",
                  "line": 1,
                  "character": 1
                }
              ],
              "implementations": [],
              "type_definitions": [],
              "references": [
                {
                  "file": "src/lib.rs",
                  "line": 1,
                  "character": 12
                }
              ]
            }
          ]
        }
      },
      {
        "inputs": {
          "file": "src/lib.rs",
          "name": "my_lib_fn",
          "line": 7,
          "character": 1
        },
        "response": {
          "info": [
            {
              "token": {
                "location": {
                  "file": "src/lib.rs",
                  "line": 14,
                  "character": 8
                },
                "token_type": "function",
                "modifiers": [
                  "declaration",
                  "public"
                ]
              },
              "hover": [
                {
                  "value": "```rust\nmain_lib\n```\n\n```rust\npub fn my_lib_fn(left: u64, right: u64) -> u64\n```\n\n---\n\nCalculate a few things.\n\n```rust\nuse main_lib::my_lib_fn;\n\nmy_lib_fn(1, 2);\n```"
                }
              ],
              "declarations": [
                {
                  "file": "src/lib.rs",
                  "line": 14,
                  "character": 8
                }
              ],
              "definitions": [
                {
                  "file": "src/lib.rs",
                  "line": 14,
                  "character": 8
                }
              ],
              "implementations": [],
              "type_definitions": [],
              "references": []
            }
          ]
        }
      },
      {
        "inputs": {
          "file": "src/lib.rs",
          "name": "accu",
          "line": 15,
          "character": 9
        },
        "response": {
          "info": [
            {
              "token": {
                "location": {
                  "file": "src/lib.rs",
                  "line": 15,
                  "character": 9
                },
                "token_type": "variable",
                "modifiers": [
                  "declaration"
                ]
              },
              "hover": [
                {
                  "language": "rust",
                  "value": "let accu: u64"
                }
              ],
              "declarations": [
                {
                  "file": "src/lib.rs",
                  "line": 15,
                  "character": 9
                }
              ],
              "definitions": [
                {
                  "file": "src/lib.rs",
                  "line": 15,
                  "character": 9
                }
              ],
              "implementations": [],
              "type_definitions": [],
              "references": [
                {
                  "file": "src/lib.rs",
                  "line": 16,
                  "character": 16
                }
              ]
            }
          ]
        }
      },
      {
        "inputs": {
          "file": "src/lib.rs",
          "name": "accu",
          "line": 16,
          "character": 9
        },
        "response": {
          "info": [
            {
              "token": {
                "location": {
                  "file": "src/lib.rs",
                  "line": 16,
                  "character": 9
                },
                "token_type": "variable",
                "modifiers": [
                  "declaration"
                ]
              },
              "hover": [
                {
                  "language": "rust",
                  "value": "let accu: u64"
                }
              ],
              "declarations": [
                {
                  "file": "src/lib.rs",
                  "line": 16,
                  "character": 9
                }
              ],
              "definitions": [
                {
                  "file": "src/lib.rs",
                  "line": 16,
                  "character": 9
                }
              ],
              "implementations": [],
              "type_definitions": [],
              "references": [
                {
                  "file": "src/lib.rs",
                  "line": 17,
                  "character": 16
                }
              ]
            }
          ]
        }
      },
      {
        "inputs": {
          "file": "src/lib.rs",
          "name": "accu",
          "line": 17,
          "character": 9
        },
        "response": {
          "info": [
            {
              "token": {
                "location": {
                  "file": "src/lib.rs",
                  "line": 17,
                  "character": 9
                },
                "token_type": "variable",
                "modifiers": [
                  "declaration"
                ]
              },
              "hover": [
                {
                  "language": "rust",
                  "value": "let accu: u64"
                }
              ],
              "declarations": [
                {
                  "file": "src/lib.rs",
                  "line": 17,
                  "character": 9
                }
              ],
              "definitions": [
                {
                  "file": "src/lib.rs",
                  "line": 17,
                  "character": 9
                }
              ],
              "implementations": [],
              "type_definitions": [],
              "references": [
                {
                  "file": "src/lib.rs",
                  "line": 18,
                  "character": 16
                }
              ]
            }
          ]
        }
      },
      {
        "inputs": {
          "file": "src/lib.rs",
          "name": "accu",
          "line": 18,
          "character": 9
        },
        "response": {
          "info": [
            {
              "token": {
                "location": {
                  "file": "src/lib.rs",
                  "line": 18,
                  "character": 9
                },
                "token_type": "variable",
                "modifiers": [
                  "declaration"
                ]
              },
              "hover": [
                {
                  "language": "rust",
                  "value": "let accu: u64"
                }
              ],
              "declarations": [
                {
                  "file": "src/lib.rs",
                  "line": 18,
                  "character": 9
                }
              ],
              "definitions": [
                {
                  "file": "src/lib.rs",
                  "line": 18,
                  "character": 9
                }
              ],
              "implementations": [],
              "type_definitions": [],
              "references": [
                {
                  "file": "src/lib.rs",
                  "line": 19,
                  "character": 5
                }
              ]
            }
          ]
        }
      },
      {
        "inputs": {
          "file": "src/lib.rs",
          "name": "my_private_lib_fn",
          "line": 22,
          "character": 1
        },
        "response": {
          "info": [
            {
              "token": {
                "location": {
                  "file": "src/lib.rs",
                  "line": 23,
                  "character": 4
                },
                "token_type": "function",
                "modifiers": [
                  "declaration"
                ]
              },
              "hover": [
                {
                  "value": "```rust\nmain_lib\n```\n\n```rust\nfn my_private_lib_fn() -> u64\n```\n\n---\n\nA private function that returns a constant value."
                }
              ],
              "declarations": [
                {
                  "file": "src/lib.rs",
                  "line": 23,
                  "character": 4
                }
              ],
              "definitions": [
                {
                  "file": "src/lib.rs",
                  "line": 23,
                  "character": 4
                }
              ],
              "implementations": [],
              "type_definitions": [],
              "references": [
                {
                  "file": "src/lib.rs",
                  "line": 18,
                  "character": 41
                }
              ]
            }
          ]
        }
      },
      {
        "inputs": {
          "file": "src/lib.rs",
          "name": "foo",
          "line": 27,
          "character": 1
        },
        "response": {
          "info": [
            {
              "token": {
                "location": {
                  "file": "src/lib.rs",
                  "line": 28,
                  "character": 4
                },
                "token_type": "function",
                "modifiers": [
                  "declaration"
                ]
              },
              "hover": [
                {
                  "value": "```rust\nmain_lib\n```\n\n```rust\nfn foo() -> u64\n```\n\n---\n\nAnother private function that returns a constant value."
                }
              ],
              "declarations": [
                {
                  "file": "src/lib.rs",
                  "line": 28,
                  "character": 4
                }
              ],
              "definitions": [
                {
                  "file": "src/lib.rs",
                  "line": 28,
                  "character": 4
                }
              ],
              "implementations": [],
              "type_definitions": [],
              "references": [
                {
                  "file": "src/lib.rs",
                  "line": 18,
                  "character": 63
                }
              ]
            }
          ]
        }
      },
      {
        "inputs": {
          "file": "src/lib.rs",
          "name": "main",
          "line": 32,
          "character": 1
        },
        "response": {
          "info": [
            {
              "token": {
                "location": {
                  "file": "src/lib.rs",
                  "line": 32,
                  "character": 4
                },
                "token_type": "function",
                "modifiers": [
                  "declaration"
                ]
              },
              "hover": [
                {
                  "language": "rust",
                  "value": "main_lib"
                },
                {
                  "language": "rust",
                  "value": "fn main()"
                }
              ],
              "declarations": [
                {
                  "file": "src/lib.rs",
                  "line": 32,
                  "character": 4
                }
              ],
              "definitions": [
                {
                  "file": "src/lib.rs",
                  "line": 32,
                  "character": 4
                }
              ],
              "implementations": [],
              "type_definitions": [],
              "references": []
            }
          ]
        }
      },
      {
        "inputs": {
          "file": "src/lib.rs",
          "name": "MyMainStruct",
          "line": 36,
          "character": 1
        },
        "response": {
          "info": [
            {
              "token": {
                "location": {
                  "file": "src/lib.rs",
                  "line": 39,
                  "character": 19
                },
                "token_type": "struct",
                "modifiers": [
                  "declaration"
                ]
              },
              "hover": [
                {
                  "value": "```rust\nmain_lib\n```\n\n```rust\npub(crate) struct MyMainStruct {\n    pub field: u64,\n}\n```\n\n---\n\nA struct that \"shadows\" the `main` function.\n\nSee <https://github.com/rust-lang/rust-analyzer/issues/19486#issuecomment-2817393342>."
                }
              ],
              "declarations": [
                {
                  "file": "src/lib.rs",
                  "line": 39,
                  "character": 19
                }
              ],
              "definitions": [
                {
                  "file": "src/lib.rs",
                  "line": 39,
                  "character": 19
                }
              ],
              "implementations": [],
              "type_definitions": [],
              "references": []
            }
          ]
        }
      },
      {
        "inputs": {
          "file": "src/lib.rs",
          "name": "field",
          "line": 40,
          "character": 5
        },
        "response": {
          "info": [
            {
              "token": {
                "location": {
                  "file": "src/lib.rs",
                  "line": 40,
                  "character": 9
                },
                "token_type": "property",
                "modifiers": [
                  "declaration",
                  "public"
                ]
              },
              "hover": [
                {
                  "language": "rust",
                  "value": "main_lib::MyMainStruct"
                },
                {
                  "language": "rust",
                  "value": "pub field: u64"
                }
              ],
              "declarations": [
                {
                  "file": "src/lib.rs",
                  "line": 40,
                  "character": 9
                }
              ],
              "definitions": [
                {
                  "file": "src/lib.rs",
                  "line": 40,
                  "character": 9
                }
              ],
              "implementations": [],
              "type_definitions": [],
              "references": []
            }
          ]
        }
      }
    ]
    "#);

    setup.shutdown().await;
}

#[tokio::test]
async fn test_multi_match() {
    let setup = TestSetup::new().await;

    let file = "src/lib.rs";

    let results = setup
        .symbol_info_ok(map([("file", json!(file)), ("name", json!("accu"))]))
        .await;
    insta::assert_json_snapshot!(results, @r#"
    {
      "info": [
        {
          "token": {
            "location": {
              "file": "src/lib.rs",
              "line": 15,
              "character": 9
            },
            "token_type": "variable",
            "modifiers": [
              "declaration"
            ]
          },
          "hover": [
            {
              "language": "rust",
              "value": "let accu: u64"
            }
          ],
          "declarations": [
            {
              "file": "src/lib.rs",
              "line": 15,
              "character": 9
            }
          ],
          "definitions": [
            {
              "file": "src/lib.rs",
              "line": 15,
              "character": 9
            }
          ],
          "implementations": [],
          "type_definitions": [],
          "references": [
            {
              "file": "src/lib.rs",
              "line": 16,
              "character": 16
            }
          ]
        },
        {
          "token": {
            "location": {
              "file": "src/lib.rs",
              "line": 16,
              "character": 9
            },
            "token_type": "variable",
            "modifiers": [
              "declaration"
            ]
          },
          "hover": [
            {
              "language": "rust",
              "value": "let accu: u64"
            }
          ],
          "declarations": [
            {
              "file": "src/lib.rs",
              "line": 16,
              "character": 9
            }
          ],
          "definitions": [
            {
              "file": "src/lib.rs",
              "line": 16,
              "character": 9
            }
          ],
          "implementations": [],
          "type_definitions": [],
          "references": [
            {
              "file": "src/lib.rs",
              "line": 17,
              "character": 16
            }
          ]
        },
        {
          "token": {
            "location": {
              "file": "src/lib.rs",
              "line": 17,
              "character": 9
            },
            "token_type": "variable",
            "modifiers": [
              "declaration"
            ]
          },
          "hover": [
            {
              "language": "rust",
              "value": "let accu: u64"
            }
          ],
          "declarations": [
            {
              "file": "src/lib.rs",
              "line": 17,
              "character": 9
            }
          ],
          "definitions": [
            {
              "file": "src/lib.rs",
              "line": 17,
              "character": 9
            }
          ],
          "implementations": [],
          "type_definitions": [],
          "references": [
            {
              "file": "src/lib.rs",
              "line": 18,
              "character": 16
            }
          ]
        },
        {
          "token": {
            "location": {
              "file": "src/lib.rs",
              "line": 18,
              "character": 9
            },
            "token_type": "variable",
            "modifiers": [
              "declaration"
            ]
          },
          "hover": [
            {
              "language": "rust",
              "value": "let accu: u64"
            }
          ],
          "declarations": [
            {
              "file": "src/lib.rs",
              "line": 18,
              "character": 9
            }
          ],
          "definitions": [
            {
              "file": "src/lib.rs",
              "line": 18,
              "character": 9
            }
          ],
          "implementations": [],
          "type_definitions": [],
          "references": [
            {
              "file": "src/lib.rs",
              "line": 19,
              "character": 5
            }
          ]
        }
      ]
    }
    "#);

    setup.shutdown().await;
}

#[tokio::test]
async fn test_foreign_symbol() {
    let setup = TestSetup::new().await.with_normalize_paths(false);

    let name = "my_lib_fn";

    let files = setup
        .find_symbol_ok(map([
            ("query", json!(name)),
            ("workspace_and_dependencies", json!(true)),
        ]))
        .await;
    let files = files["symbols"]
        .as_array()
        .expect("symbols array")
        .iter()
        .map(|map| {
            map["location"]["file"]
                .as_str()
                .expect("should be string")
                .to_owned()
        })
        .filter(|file| file.starts_with("/"))
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 1);
    let file = &files[0];
    println!("file: {file}");

    let setup = setup.with_normalize_paths(true);

    let results = setup
        .symbol_info_ok(map([("file", json!(file)), ("name", json!(name))]))
        .await;
    insta::assert_json_snapshot!(results, @r#"
    {
      "info": [
        {
          "token": {
            "location": {
              "file": "/fixtures/dependency_lib/src/lib.rs",
              "line": 1,
              "character": 8
            },
            "token_type": "function",
            "modifiers": [
              "declaration",
              "public"
            ]
          },
          "hover": [
            {
              "language": "rust",
              "value": "dependency_lib"
            },
            {
              "language": "rust",
              "value": "pub fn my_lib_fn(left: u64, right: u64) -> u64"
            }
          ],
          "declarations": [],
          "definitions": [],
          "implementations": [],
          "type_definitions": [],
          "references": [
            {
              "file": "src/lib.rs",
              "line": 2,
              "character": 21
            }
          ]
        }
      ]
    }
    "#);

    setup.shutdown().await;
}

#[tokio::test]
async fn test_file_not_found() {
    let setup = TestSetup::new().await.with_normalize_paths(false);

    let results = setup
        .symbol_info(map([
            ("file", json!("does_not_exist.rs")),
            ("name", json!("foo")),
        ]))
        .await
        .unwrap_err();
    insta::assert_json_snapshot!(results, @r#"
    {
      "code": -32602,
      "message": "file not found: does_not_exist.rs"
    }
    "#);

    setup.shutdown().await;
}
