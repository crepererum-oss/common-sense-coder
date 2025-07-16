use crate::setup::{TestSetup, map};
use serde_json::json;

#[tokio::test]
async fn test_workspace_query() {
    let setup = TestSetup::new().await;

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("query", json!("does_not_exist")),
        ])).await,
        @"[]",
    );

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("query", json!("my_lib_fn")),
        ])).await,
        @r#"
    [
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 6,
        "character": 8
      }
    ]
    "#,
    );

    // should NOT find library function
    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("query", json!("mylibfn")),
        ])).await,
        @"[]",
    );
}

#[tokio::test]
async fn test_global_query() {
    let setup = TestSetup::new().await;

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("query", json!("my_lib_fn")),
            ("workspace_and_dependencies", json!(true)),
        ])).await,
        @r#"
    [
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "/fixtures/dependency_lib/src/lib.rs",
        "line": 1,
        "character": 8
      },
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 6,
        "character": 8
      }
    ]
    "#,
    );

    // query is NOT fuzzy
    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("query", json!("mylibfn")),
            ("workspace_and_dependencies", json!(true)),
        ])).await,
        @"[]",
    );
}

#[tokio::test]
async fn test_workspace_fuzzy_query() {
    let setup = TestSetup::new().await;

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("query", json!("mylibfn")),
            ("fuzzy", json!(true)),
        ])).await,
        @r#"
    [
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 6,
        "character": 8
      },
      {
        "type": "json",
        "name": "my_sub_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/sub.rs",
        "line": 1,
        "character": 15
      },
      {
        "type": "json",
        "name": "my_sub_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 1,
        "character": 17
      }
    ]
    "#,
    );
}

#[tokio::test]
async fn test_global_fuzzy_query() {
    let setup = TestSetup::new().await;

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("query", json!("mylibfn")),
            ("fuzzy", json!(true)),
            ("workspace_and_dependencies", json!(true)),
        ])).await,
        @r#"
    [
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "/fixtures/dependency_lib/src/lib.rs",
        "line": 1,
        "character": 8
      },
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 6,
        "character": 8
      },
      {
        "type": "json",
        "name": "my_sub_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/sub.rs",
        "line": 1,
        "character": 15
      },
      {
        "type": "json",
        "name": "my_sub_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 1,
        "character": 17
      }
    ]
    "#,
    );
}

#[tokio::test]
async fn test_path() {
    let setup = TestSetup::new().await;

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("path", json!("src/lib.rs")),
        ])).await,
        @r#"
    [
      {
        "type": "json",
        "name": "sub",
        "kind": "Module",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 4,
        "character": 1
      },
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 6,
        "character": 1
      }
    ]
    "#
    );

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("path", json!("src/sub.rs")),
        ])).await,
        @r#"
    [
      {
        "type": "json",
        "name": "my_sub_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/sub.rs",
        "line": 1,
        "character": 1
      }
    ]
    "#
    );
}

#[tokio::test]
async fn test_path_query() {
    let setup = TestSetup::new().await;

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("path", json!("src/lib.rs")),
            ("query", json!("does_not_exist")),
        ])).await,
        @"[]"
    );

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("path", json!("src/lib.rs")),
            ("query", json!("my_lib_fn")),
        ])).await,
        @r#"
    [
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 6,
        "character": 1
      }
    ]
    "#
    );

    // query is NOT fuzzy
    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("path", json!("src/lib.rs")),
            ("query", json!("mylibfn")),
        ])).await,
        @"[]",
    );
}

#[tokio::test]
async fn test_path_fuzzy_query() {
    let setup = TestSetup::new().await;

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("path", json!("src/lib.rs")),
            ("query", json!("mylibfn")),
            ("fuzzy", json!(true)),
        ])).await,
        @r#"
    [
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 6,
        "character": 1
      }
    ]
    "#,
    );
}
