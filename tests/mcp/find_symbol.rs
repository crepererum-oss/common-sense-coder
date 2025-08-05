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
        "file": "unused_workspace_member/src/lib.rs",
        "line": 1,
        "character": 8
      },
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "workspace_member/src/lib.rs",
        "line": 1,
        "character": 8
      },
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 14,
        "character": 8
      }
    ]
    "#,
    );

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("query", json!("my_private_lib_fn")),
        ])).await,
        @r#"
    [
      {
        "type": "json",
        "name": "my_private_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 23,
        "character": 4
      }
    ]
    "#,
    );

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("query", json!("main")),
        ])).await,
        @r#"
    [
      {
        "type": "json",
        "name": "main",
        "kind": "Function",
        "deprecated": false,
        "file": "unused_workspace_member/build.rs",
        "line": 1,
        "character": 4
      },
      {
        "type": "json",
        "name": "main",
        "kind": "Function",
        "deprecated": false,
        "file": "build.rs",
        "line": 1,
        "character": 4
      },
      {
        "type": "json",
        "name": "main",
        "kind": "Function",
        "deprecated": false,
        "file": "workspace_member/build.rs",
        "line": 1,
        "character": 4
      },
      {
        "type": "json",
        "name": "main",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 32,
        "character": 4
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

    setup.shutdown().await;
}

#[tokio::test]
async fn test_global_query() {
    let setup = TestSetup::new().await;
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

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
        "file": "unused_workspace_member/src/lib.rs",
        "line": 1,
        "character": 8
      },
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
        "file": "workspace_member/src/lib.rs",
        "line": 1,
        "character": 8
      },
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 14,
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

    setup.shutdown().await;
}

#[tokio::test]
async fn test_fallback_to_global_query() {
    let setup = TestSetup::new().await;

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("query", json!("my_unused_lib_fn")),
        ])).await,
        @r#"
    [
      {
        "type": "json",
        "name": "my_unused_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "/fixtures/dependency_lib/src/lib.rs",
        "line": 5,
        "character": 8
      }
    ]
    "#,
    );

    // does NOT fall back if scope is explicitely local
    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("query", json!("my_unused_lib_fn")),
            ("workspace_and_dependencies", json!(false)),
        ])).await,
        @"[]",
    );

    setup.shutdown().await;
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
        "file": "unused_workspace_member/src/lib.rs",
        "line": 1,
        "character": 8
      },
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "workspace_member/src/lib.rs",
        "line": 1,
        "character": 8
      },
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 14,
        "character": 8
      },
      {
        "type": "json",
        "name": "my_private_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 23,
        "character": 4
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

    setup.shutdown().await;
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
        "file": "unused_workspace_member/src/lib.rs",
        "line": 1,
        "character": 8
      },
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
        "file": "workspace_member/src/lib.rs",
        "line": 1,
        "character": 8
      },
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 14,
        "character": 8
      },
      {
        "type": "json",
        "name": "my_private_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 23,
        "character": 4
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
      },
      {
        "type": "json",
        "name": "my_unused_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "/fixtures/dependency_lib/src/lib.rs",
        "line": 5,
        "character": 8
      }
    ]
    "#,
    );

    setup.shutdown().await;
}

#[tokio::test]
async fn test_file() {
    let setup = TestSetup::new().await;

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("file", json!("src/lib.rs")),
        ])).await,
        @r#"
    [
      {
        "type": "json",
        "name": "sub",
        "kind": "Module",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 5,
        "character": 1
      },
      {
        "type": "json",
        "name": "my_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 7,
        "character": 1
      },
      {
        "type": "json",
        "name": "accu",
        "kind": "Variable",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 15,
        "character": 5
      },
      {
        "type": "json",
        "name": "accu",
        "kind": "Variable",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 16,
        "character": 5
      },
      {
        "type": "json",
        "name": "accu",
        "kind": "Variable",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 17,
        "character": 5
      },
      {
        "type": "json",
        "name": "accu",
        "kind": "Variable",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 18,
        "character": 5
      },
      {
        "type": "json",
        "name": "my_private_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 22,
        "character": 1
      },
      {
        "type": "json",
        "name": "foo",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 27,
        "character": 1
      },
      {
        "type": "json",
        "name": "main",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 32,
        "character": 1
      },
      {
        "type": "json",
        "name": "MyMainStruct",
        "kind": "Struct",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 36,
        "character": 1
      },
      {
        "type": "json",
        "name": "field",
        "kind": "Field",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 40,
        "character": 5
      }
    ]
    "#
    );

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("file", json!("src/sub.rs")),
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

    insta::assert_json_snapshot!(
        setup.find_symbol(map([
            ("file", json!("does_not_exist.rs")),
        ])).await.unwrap_err(),
        @r#"
    [
      {
        "type": "text",
        "text": "file not found: does_not_exist.rs"
      }
    ]
    "#,
    );

    setup.shutdown().await;
}

#[tokio::test]
async fn test_file_query() {
    let setup = TestSetup::new().await;

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("file", json!("src/lib.rs")),
            ("query", json!("does_not_exist")),
        ])).await,
        @"[]"
    );

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("file", json!("src/lib.rs")),
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
        "line": 7,
        "character": 1
      }
    ]
    "#
    );

    // query is NOT fuzzy
    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("file", json!("src/lib.rs")),
            ("query", json!("mylibfn")),
        ])).await,
        @"[]",
    );

    setup.shutdown().await;
}

#[tokio::test]
async fn test_file_fuzzy_query() {
    let setup = TestSetup::new().await;

    insta::assert_json_snapshot!(
        setup.find_symbol_ok(map([
            ("file", json!("src/lib.rs")),
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
        "line": 7,
        "character": 1
      },
      {
        "type": "json",
        "name": "my_private_lib_fn",
        "kind": "Function",
        "deprecated": false,
        "file": "src/lib.rs",
        "line": 22,
        "character": 1
      }
    ]
    "#,
    );

    setup.shutdown().await;
}
