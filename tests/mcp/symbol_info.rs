use std::fmt::Write;

use serde_json::json;

use crate::setup::{TestSetup, TextOrJson, map};

const RESULT_SEP: &str = "==========";

#[tokio::test]
async fn test_info_for_all_in_file() {
    let setup = TestSetup::new().await;

    let path = "src/lib.rs";

    let symbols = setup.find_symbol_ok(map([("path", json!(path))])).await;

    let mut snapshot = String::new();
    for symbol in symbols {
        writeln!(&mut snapshot).unwrap();
        writeln!(&mut snapshot, "{RESULT_SEP}").unwrap();
        writeln!(&mut snapshot).unwrap();

        let (name, line, character) = match symbol {
            TextOrJson::Text(_) => panic!("should be JSON"),
            TextOrJson::Json(map) => {
                let name = map
                    .get("name")
                    .expect("name")
                    .as_str()
                    .expect("str")
                    .to_owned();
                let line = map.get("line").expect("line").as_u64().expect("u64");
                let character = map
                    .get("character")
                    .expect("character")
                    .as_u64()
                    .expect("u64");
                (name, line, character)
            }
        };

        writeln!(&mut snapshot, "Inputs:").unwrap();
        writeln!(&mut snapshot, "  path: {path}").unwrap();
        writeln!(&mut snapshot, "  name: {name}").unwrap();
        writeln!(&mut snapshot, "  line: {line}").unwrap();
        writeln!(&mut snapshot, "  char: {character}").unwrap();
        writeln!(&mut snapshot).unwrap();
        writeln!(&mut snapshot, "---").unwrap();
        writeln!(&mut snapshot).unwrap();

        let resp = setup
            .symbol_info_ok(map([
                ("path", json!(path)),
                ("name", json!(name)),
                ("line", json!(line)),
                ("character", json!(character)),
            ]))
            .await;

        for part in resp {
            writeln!(&mut snapshot, "{part}").unwrap();
        }
    }

    insta::assert_snapshot!(snapshot, @r"
    ==========

    Inputs:
      path: src/lib.rs
      name: sub
      line: 4
      char: 1

    ---

    Token:

    - location: src/lib.rs:4:5
    - type: namespace
    - modifiers: declaration

    ---

    ```rust
    main_lib
    ```

    ```rust
    mod sub
    ```

    ---

    Declaration:
    - src/lib.rs:4:5

    ---

    Definition:
    - src/sub.rs:1:1

    ---

    Implementation:
    None

    ---

    Type Definition:
    None

    ---

    References:
    - src/lib.rs:1:12

    ==========

    Inputs:
      path: src/lib.rs
      name: my_lib_fn
      line: 6
      char: 1

    ---

    Token:

    - location: src/lib.rs:6:8
    - type: function
    - modifiers: declaration, public

    ---

    ```rust
    main_lib
    ```

    ```rust
    pub fn my_lib_fn(left: u64, right: u64) -> u64
    ```

    ---

    Declaration:
    - src/lib.rs:6:8

    ---

    Definition:
    - src/lib.rs:6:8

    ---

    Implementation:
    None

    ---

    Type Definition:
    None

    ---

    References:
    None

    ==========

    Inputs:
      path: src/lib.rs
      name: accu
      line: 7
      char: 5

    ---

    Token:

    - location: src/lib.rs:7:9
    - type: variable
    - modifiers: declaration

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declaration:
    - src/lib.rs:7:9

    ---

    Definition:
    - src/lib.rs:7:9

    ---

    Implementation:
    None

    ---

    Type Definition:
    None

    ---

    References:
    - src/lib.rs:8:16

    ==========

    Inputs:
      path: src/lib.rs
      name: accu
      line: 8
      char: 5

    ---

    Token:

    - location: src/lib.rs:8:9
    - type: variable
    - modifiers: declaration

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declaration:
    - src/lib.rs:8:9

    ---

    Definition:
    - src/lib.rs:8:9

    ---

    Implementation:
    None

    ---

    Type Definition:
    None

    ---

    References:
    - src/lib.rs:9:16

    ==========

    Inputs:
      path: src/lib.rs
      name: accu
      line: 9
      char: 5

    ---

    Token:

    - location: src/lib.rs:9:9
    - type: variable
    - modifiers: declaration

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declaration:
    - src/lib.rs:9:9

    ---

    Definition:
    - src/lib.rs:9:9

    ---

    Implementation:
    None

    ---

    Type Definition:
    None

    ---

    References:
    - src/lib.rs:10:5

    ==========

    Inputs:
      path: src/lib.rs
      name: private_fn
      line: 13
      char: 1

    ---

    Token:

    - location: src/lib.rs:14:4
    - type: function
    - modifiers: declaration

    ---

    ```rust
    main_lib
    ```

    ```rust
    fn private_fn() -> u64
    ```

    ---

    A private function that returns a constant value.

    ---

    Declaration:
    - src/lib.rs:14:4

    ---

    Definition:
    - src/lib.rs:14:4

    ---

    Implementation:
    None

    ---

    Type Definition:
    None

    ---

    References:
    - src/lib.rs:9:41
    ");
}

#[tokio::test]
async fn test_multi_match() {
    let setup = TestSetup::new().await;

    let path = "src/lib.rs";

    let results = setup
        .symbol_info_ok(map([("path", json!(path)), ("name", json!("accu"))]))
        .await;
    let results = results.join(&format!("\n\n{RESULT_SEP}\n\n"));
    insta::assert_snapshot!(results, @r"
    Token:

    - location: src/lib.rs:7:9
    - type: variable
    - modifiers: declaration

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declaration:
    - src/lib.rs:7:9

    ---

    Definition:
    - src/lib.rs:7:9

    ---

    Implementation:
    None

    ---

    Type Definition:
    None

    ---

    References:
    - src/lib.rs:8:16

    ==========

    Token:

    - location: src/lib.rs:8:9
    - type: variable
    - modifiers: declaration

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declaration:
    - src/lib.rs:8:9

    ---

    Definition:
    - src/lib.rs:8:9

    ---

    Implementation:
    None

    ---

    Type Definition:
    None

    ---

    References:
    - src/lib.rs:9:16

    ==========

    Token:

    - location: src/lib.rs:8:16
    - type: variable
    - modifiers: none

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declaration:
    - src/lib.rs:7:9

    ---

    Definition:
    - src/lib.rs:7:9

    ---

    Implementation:
    None

    ---

    Type Definition:
    None

    ---

    References:
    - src/lib.rs:8:16

    ==========

    Token:

    - location: src/lib.rs:9:9
    - type: variable
    - modifiers: declaration

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declaration:
    - src/lib.rs:9:9

    ---

    Definition:
    - src/lib.rs:9:9

    ---

    Implementation:
    None

    ---

    Type Definition:
    None

    ---

    References:
    - src/lib.rs:10:5

    ==========

    Token:

    - location: src/lib.rs:9:16
    - type: variable
    - modifiers: none

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declaration:
    - src/lib.rs:8:9

    ---

    Definition:
    - src/lib.rs:8:9

    ---

    Implementation:
    None

    ---

    Type Definition:
    None

    ---

    References:
    - src/lib.rs:9:16

    ==========

    Token:

    - location: src/lib.rs:10:5
    - type: variable
    - modifiers: none

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declaration:
    - src/lib.rs:9:9

    ---

    Definition:
    - src/lib.rs:9:9

    ---

    Implementation:
    None

    ---

    Type Definition:
    None

    ---

    References:
    - src/lib.rs:10:5
    ");
}

#[tokio::test]
async fn test_foreign_symbol() {
    let setup = TestSetup::new().await.with_normalize_paths(false);

    let name = "my_lib_fn";

    let paths = setup
        .find_symbol_ok(map([
            ("query", json!(name)),
            ("workspace_and_dependencies", json!(true)),
        ]))
        .await
        .into_iter()
        .map(|res| match res {
            TextOrJson::Text(_) => panic!("should be JSON"),
            TextOrJson::Json(map) => map
                .get("file")
                .expect("file")
                .as_str()
                .expect("should be string")
                .to_owned(),
        })
        .filter(|path| path.starts_with("/"))
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    let path = &paths[0];
    println!("path: {path}");

    let setup = setup.with_normalize_paths(true);

    let results = setup
        .symbol_info_ok(map([("path", json!(path)), ("name", json!(name))]))
        .await;
    let results = results.join(&format!("\n\n{RESULT_SEP}\n\n"));
    insta::assert_snapshot!(results, @r"");
}
