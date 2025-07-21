// crates used by main binary
use anyhow as _;
use clap as _;
use dotenvy as _;
use lsp_client as _;
use lsp_types as _;
use predicates as _;
use tracing as _;
use tracing_log as _;
use tracing_subscriber as _;

mod find_symbol;
mod setup;
mod symbol_info;
