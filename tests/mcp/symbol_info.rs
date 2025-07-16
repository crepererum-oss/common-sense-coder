use std::fmt::Write;

use serde_json::json;

use crate::setup::{TestSetup, TextOrJson, map};

#[tokio::test]
async fn test_info_for_all_in_file() {
    let setup = TestSetup::new().await;

    let path = "src/lib.rs";

    let symbols = setup.find_symbol_ok(map([("path", json!(path))])).await;

    let mut snapshot = String::new();
    for symbol in symbols {
        writeln!(&mut snapshot).unwrap();
        writeln!(&mut snapshot, "==========").unwrap();
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

    Token Location:

    src/lib.rs:4:5

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

    Token Location:

    src/lib.rs:6:8

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
      name: private_fn
      line: 10
      char: 1

    ---

    Token Location:

    src/lib.rs:11:4

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

    - src/lib.rs:11:4

    ---

    Definition:

    - src/lib.rs:11:4

    ---

    Implementation:

    None

    ---

    Type Definition:

    None

    ---

    References:

    - src/lib.rs:7:64
    ");
}
