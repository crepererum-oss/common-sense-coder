use std::{ops::Deref, path::Path, process::Stdio};

use assert_cmd::{cargo::cargo_bin, crate_name};
use rmcp::{
    RoleClient,
    model::{CallToolRequestParam, JsonObject, RawContent},
    service::{RunningService, ServiceExt},
    transport::TokioChildProcess,
};
use serde::Serialize;
use serde_json::Value;
use tempfile::TempDir;
use tokio::process::Command;

/// Temporary directory that holds IO interception data (like logs).
///
/// During a panic/test-failure it will NOT be cleaned up to simplify debugging.
#[derive(Debug)]
struct InterceptIoDir {
    dir: TempDir,
}

impl InterceptIoDir {
    /// Create new, empty dir and print out location to stdout.
    fn new() -> Self {
        let dir = if let Some(dir) = std::env::var_os("TEST_IO_INTERCEPT") {
            std::fs::create_dir_all(&dir).expect("create IO intercept dir");
            TempDir::with_prefix_in("", &dir).expect("temp dir creation")
        } else {
            TempDir::new().expect("temp dir creation")
        };
        println!("intercept IO: {}", dir.path().display());

        Self { dir }
    }
}

impl Deref for InterceptIoDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.dir.path()
    }
}

impl Drop for InterceptIoDir {
    fn drop(&mut self) {
        self.dir.disable_cleanup(std::thread::panicking());
    }
}

/// Test fixture that contains a running MCP server.
#[derive(Debug)]
pub(crate) struct TestSetup {
    fixtures_path: String,

    #[expect(dead_code)]
    intercept_io_dir: InterceptIoDir,

    #[expect(dead_code)]
    cwd: TempDir,

    service: RunningService<RoleClient, ()>,

    normalize_paths: bool,
}

impl TestSetup {
    pub(crate) async fn new() -> Self {
        let server_path = cargo_bin(crate_name!())
            .canonicalize()
            .expect("canonicalize");

        let fixtures_path = Path::new(file!())
            .parent()
            .expect("parent 1")
            .parent()
            .expect("parent 2")
            .join("fixtures")
            .canonicalize()
            .expect("canonicalize");
        let main_lib_path = fixtures_path.join("main_lib").display().to_string();

        let intercept_io_dir = InterceptIoDir::new();
        let server_stderr_path = intercept_io_dir.join("server.stderr.txt");
        println!("server stderr: {}", server_stderr_path.display());
        let server_stderr = Stdio::from(
            tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&server_stderr_path)
                .await
                .expect("open stderr log file for language server")
                .into_std()
                .await,
        );

        // add a cwd to avoid dependency on it
        let cwd = TempDir::new().expect("create CWD temp dir");

        let mut cmd = Command::new(server_path);
        cmd.current_dir(cwd.path())
            .env("RUST_BACKTRACE", "1")
            .arg("--intercept-io")
            .arg(intercept_io_dir.display().to_string())
            .arg("--workspace")
            .arg(main_lib_path)
            .arg("-vv");

        let child = TokioChildProcess::builder(cmd)
            .stderr(server_stderr)
            .spawn()
            .expect("spawn language server")
            .0;

        let service = ().serve(child).await.expect("service start");

        Self {
            fixtures_path: fixtures_path.display().to_string(),
            intercept_io_dir,
            cwd,
            service,
            normalize_paths: true,
        }
    }

    pub(crate) fn with_normalize_paths(mut self, normalize_paths: bool) -> Self {
        self.normalize_paths = normalize_paths;
        self
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParam,
    ) -> Result<Vec<TextOrJson>, Vec<TextOrJson>> {
        let resp = self.service.call_tool(params).await.expect("call tool");

        let data = resp
            .content
            .into_iter()
            .map(|annotated| match annotated.raw {
                RawContent::Text(raw_text_content) => {
                    let s = raw_text_content.text;
                    let s = if self.normalize_paths {
                        s.replace(&self.fixtures_path, "/fixtures")
                    } else {
                        s
                    };

                    TextOrJson::from(s)
                }
                RawContent::Image(_) => unimplemented!("image content not supported"),
                RawContent::Resource(_) => unimplemented!("resource content not supported"),
                RawContent::Audio(_) => unimplemented!("audio content not supported"),
            })
            .collect();

        if resp.is_error.unwrap_or_default() {
            Err(data)
        } else {
            Ok(data)
        }
    }

    pub(crate) async fn find_symbol(
        &self,
        args: JsonObject,
    ) -> Result<Vec<TextOrJson>, Vec<TextOrJson>> {
        self.call_tool(CallToolRequestParam {
            name: "find_symbol".into(),
            arguments: Some(args),
        })
        .await
    }

    pub(crate) async fn find_symbol_ok(&self, args: JsonObject) -> Vec<TextOrJson> {
        self.find_symbol(args).await.expect("no error")
    }

    pub(crate) async fn symbol_info(&self, args: JsonObject) -> Result<Vec<String>, Vec<String>> {
        let map_data = |data: Vec<TextOrJson>| {
            data.into_iter()
                .map(|res| match res {
                    TextOrJson::Text { text } => text,
                    TextOrJson::Json(_) => panic!("expected non-JSON content"),
                })
                .collect()
        };

        self.call_tool(CallToolRequestParam {
            name: "symbol_info".into(),
            arguments: Some(args),
        })
        .await
        .map(map_data)
        .map_err(map_data)
    }

    pub(crate) async fn symbol_info_ok(&self, args: JsonObject) -> Vec<String> {
        self.symbol_info(args).await.expect("no error")
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(crate) enum TextOrJson {
    Text { text: String },
    Json(JsonObject),
}

impl From<String> for TextOrJson {
    fn from(s: String) -> Self {
        match serde_json::from_str::<JsonObject>(&s) {
            Ok(obj) => Self::Json(obj),
            Err(_) => Self::Text { text: s },
        }
    }
}

pub(crate) fn map<const N: usize>(m: [(&'static str, Value); N]) -> JsonObject {
    m.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}
