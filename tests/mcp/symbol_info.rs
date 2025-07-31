use std::fmt::Write;

use serde_json::json;

use crate::setup::{TestSetup, TextOrJson, map};

const RESULT_SEP: &str = "==========";

#[tokio::test]
async fn test_info_for_all_in_file() {
    let setup = TestSetup::new().await;

    let file = "src/lib.rs";

    let symbols = setup.find_symbol_ok(map([("file", json!(file))])).await;

    let mut snapshot = String::new();
    for symbol in symbols {
        writeln!(&mut snapshot).unwrap();
        writeln!(&mut snapshot, "{RESULT_SEP}").unwrap();
        writeln!(&mut snapshot).unwrap();

        let (name, line, character) = match symbol {
            TextOrJson::Text { .. } => panic!("should be JSON"),
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
        writeln!(&mut snapshot, "  file: {file}").unwrap();
        writeln!(&mut snapshot, "  name: {name}").unwrap();
        writeln!(&mut snapshot, "  line: {line}").unwrap();
        writeln!(&mut snapshot, "  char: {character}").unwrap();
        writeln!(&mut snapshot).unwrap();
        writeln!(&mut snapshot, "---").unwrap();
        writeln!(&mut snapshot).unwrap();

        let resp = setup
            .symbol_info_ok(map([
                ("file", json!(file)),
                ("name", json!(name)),
                ("line", json!(line)),
                ("character", json!(character)),
            ]))
            .await;

        for part in resp {
            writeln!(&mut snapshot, "{part}").unwrap();
        }
    }

    insta::assert_snapshot!(snapshot, @r#"
    ==========

    Inputs:
      file: src/lib.rs
      name: sub
      line: 4
      char: 1

    ---

    Token:

    - location: {"file":"src/lib.rs","line":4,"character":5}
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

    Declarations:
    - {"file":"src/lib.rs","line":4,"character":5}

    ---

    Definitions:
    - {"file":"src/sub.rs","line":1,"character":1}

    ---

    Implementations:
    None

    ---

    Type Definitions:
    None

    ---

    References:
    - {"file":"src/lib.rs","line":1,"character":12}

    ==========

    Inputs:
      file: src/lib.rs
      name: my_lib_fn
      line: 6
      char: 1

    ---

    Token:

    - location: {"file":"src/lib.rs","line":13,"character":8}
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

    Calculate a few things.

    ```rust
    use main_lib::my_lib_fn;

    my_lib_fn(1, 2);
    ```

    ---

    Declarations:
    - {"file":"src/lib.rs","line":13,"character":8}

    ---

    Definitions:
    - {"file":"src/lib.rs","line":13,"character":8}

    ---

    Implementations:
    None

    ---

    Type Definitions:
    None

    ---

    References:
    None

    ==========

    Inputs:
      file: src/lib.rs
      name: accu
      line: 14
      char: 5

    ---

    Token:

    - location: {"file":"src/lib.rs","line":14,"character":9}
    - type: variable
    - modifiers: declaration

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declarations:
    - {"file":"src/lib.rs","line":14,"character":9}

    ---

    Definitions:
    - {"file":"src/lib.rs","line":14,"character":9}

    ---

    Implementations:
    None

    ---

    Type Definitions:
    None

    ---

    References:
    - {"file":"src/lib.rs","line":15,"character":16}

    ==========

    Inputs:
      file: src/lib.rs
      name: accu
      line: 15
      char: 5

    ---

    Token:

    - location: {"file":"src/lib.rs","line":15,"character":9}
    - type: variable
    - modifiers: declaration

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declarations:
    - {"file":"src/lib.rs","line":15,"character":9}

    ---

    Definitions:
    - {"file":"src/lib.rs","line":15,"character":9}

    ---

    Implementations:
    None

    ---

    Type Definitions:
    None

    ---

    References:
    - {"file":"src/lib.rs","line":16,"character":16}

    ==========

    Inputs:
      file: src/lib.rs
      name: accu
      line: 16
      char: 5

    ---

    Token:

    - location: {"file":"src/lib.rs","line":16,"character":9}
    - type: variable
    - modifiers: declaration

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declarations:
    - {"file":"src/lib.rs","line":16,"character":9}

    ---

    Definitions:
    - {"file":"src/lib.rs","line":16,"character":9}

    ---

    Implementations:
    None

    ---

    Type Definitions:
    None

    ---

    References:
    - {"file":"src/lib.rs","line":17,"character":5}

    ==========

    Inputs:
      file: src/lib.rs
      name: private_fn
      line: 20
      char: 1

    ---

    Token:

    - location: {"file":"src/lib.rs","line":21,"character":4}
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

    Declarations:
    - {"file":"src/lib.rs","line":21,"character":4}

    ---

    Definitions:
    - {"file":"src/lib.rs","line":21,"character":4}

    ---

    Implementations:
    None

    ---

    Type Definitions:
    None

    ---

    References:
    - {"file":"src/lib.rs","line":16,"character":41}
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
    let results = results.join(&format!("\n\n{RESULT_SEP}\n\n"));
    insta::assert_snapshot!(results, @r#"
    Token:

    - location: {"file":"src/lib.rs","line":14,"character":9}
    - type: variable
    - modifiers: declaration

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declarations:
    - {"file":"src/lib.rs","line":14,"character":9}

    ---

    Definitions:
    - {"file":"src/lib.rs","line":14,"character":9}

    ---

    Implementations:
    None

    ---

    Type Definitions:
    None

    ---

    References:
    - {"file":"src/lib.rs","line":15,"character":16}

    ==========

    Token:

    - location: {"file":"src/lib.rs","line":15,"character":9}
    - type: variable
    - modifiers: declaration

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declarations:
    - {"file":"src/lib.rs","line":15,"character":9}

    ---

    Definitions:
    - {"file":"src/lib.rs","line":15,"character":9}

    ---

    Implementations:
    None

    ---

    Type Definitions:
    None

    ---

    References:
    - {"file":"src/lib.rs","line":16,"character":16}

    ==========

    Token:

    - location: {"file":"src/lib.rs","line":16,"character":9}
    - type: variable
    - modifiers: declaration

    ---

    ```rust
    let accu: u64
    ```

    ---

    Declarations:
    - {"file":"src/lib.rs","line":16,"character":9}

    ---

    Definitions:
    - {"file":"src/lib.rs","line":16,"character":9}

    ---

    Implementations:
    None

    ---

    Type Definitions:
    None

    ---

    References:
    - {"file":"src/lib.rs","line":17,"character":5}
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
        .await
        .into_iter()
        .map(|res| match res {
            TextOrJson::Text { .. } => panic!("should be JSON"),
            TextOrJson::Json(map) => map
                .get("file")
                .expect("file")
                .as_str()
                .expect("should be string")
                .to_owned(),
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
    let results = results.join(&format!("\n\n{RESULT_SEP}\n\n"));
    insta::assert_snapshot!(results, @r#"
    Token:

    - location: {"file":"/fixtures/dependency_lib/src/lib.rs","line":1,"character":8}
    - type: function
    - modifiers: declaration, public

    ---

    ```rust
    dependency_lib
    ```

    ```rust
    pub fn my_lib_fn(left: u64, right: u64) -> u64
    ```

    ---

    Declarations:
    None

    ---

    Definitions:
    None

    ---

    Implementations:
    None

    ---

    Type Definitions:
    None

    ---

    References:
    - {"file":"src/lib.rs","line":2,"character":21}
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
    let results = results.join(&format!("\n\n{RESULT_SEP}\n\n"));
    insta::assert_snapshot!(results, @"file not found: does_not_exist.rs");

    setup.shutdown().await;
}
