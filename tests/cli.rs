// crates used by main binary or other helps
use anyhow as _;
use clap as _;
use dotenvy as _;
use futures as _;
use insta as _;
use lsp_client as _;
use lsp_types as _;
use rmcp as _;
use serde as _;
use serde_json as _;
use tokio as _;
use tracing as _;
use tracing_log as _;
use tracing_subscriber as _;

use assert_cmd::{Command, crate_name};
use tempfile::TempDir;

#[test]
fn test_help_arg() {
    Command::cargo_bin(crate_name!())
        .unwrap()
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn test_version_arg() {
    Command::cargo_bin(crate_name!())
        .unwrap()
        .arg("--version")
        .assert()
        .success();
}

#[test]
fn test_dotenv_not_found() {
    let cwd = TempDir::new().unwrap();
    Command::cargo_bin(crate_name!())
        .unwrap()
        .current_dir(cwd.path())
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn test_dotenv_invalid() {
    let cwd = TempDir::new().unwrap();
    std::fs::write(cwd.path().join(".env"), "X").unwrap();
    Command::cargo_bin(crate_name!())
        .unwrap()
        .current_dir(cwd.path())
        .arg("--help")
        .assert()
        .failure()
        .stderr(predicates::str::contains("Error parsing line"));
}
