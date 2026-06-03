use std::{ops::Deref, path::Path, process::Stdio};

use assert_cmd::{cargo::cargo_bin, pkg_name};
use rmcp::{
    RoleClient,
    model::{CallToolRequestParams, JsonObject, Tool},
    service::{RunningService, ServiceError, ServiceExt},
    transport::TokioChildProcess,
};
use serde_json::Value;
use tempfile::TempDir;
use tokio::process::Command;

/// Temporary directory that holds IO interception data (like logs).
///
/// During a panic/test-failure it will NOT be cleaned up to simplify debugging.
#[derive(Debug)]
struct InterceptIoDir {
    dir: TempDir,
    disable_cleanup: Option<bool>,
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

        Self {
            dir,
            disable_cleanup: None,
        }
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
        self.dir
            .disable_cleanup(self.disable_cleanup.unwrap_or(std::thread::panicking()));
    }
}

/// Test fixture that contains a running MCP server.
#[derive(Debug)]
pub(crate) struct TestSetup {
    fixtures_path: String,

    intercept_io_dir: InterceptIoDir,

    #[expect(dead_code)]
    cwd: TempDir,

    service: Option<RunningService<RoleClient, ()>>,

    normalize_paths: bool,
}

impl TestSetup {
    pub(crate) async fn new() -> Self {
        let server_path = cargo_bin(pkg_name!()).canonicalize().expect("canonicalize");

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
            service: Some(service),
            normalize_paths: true,
        }
    }

    pub(crate) fn with_normalize_paths(mut self, normalize_paths: bool) -> Self {
        self.normalize_paths = normalize_paths;
        self
    }

    pub(crate) async fn list_all_tools(&self) -> Vec<Tool> {
        self.service
            .as_ref()
            .expect("not shut down")
            .list_all_tools()
            .await
            .expect("can list tools")
    }

    async fn call_tool(&self, params: CallToolRequestParams) -> Result<Value, Value> {
        let resp = match self
            .service
            .as_ref()
            .expect("not shut down")
            .call_tool(params)
            .await
        {
            Ok(resp) => resp,
            Err(ServiceError::McpError(error)) => {
                return Err(serde_json::to_value(error).expect("serialize MCP error"));
            }
            Err(error) => panic!("call tool: {error}"),
        };

        let mut data = resp
            .structured_content
            .expect("tool result should always have structured content");

        if self.normalize_paths {
            data = normalize_paths(data, &self.fixtures_path);
        }

        if resp.is_error.unwrap_or_default() {
            Err(data)
        } else {
            Ok(data)
        }
    }

    pub(crate) async fn find_symbol(&self, args: JsonObject) -> Result<Value, Value> {
        self.call_tool(CallToolRequestParams::new("find_symbol").with_arguments(args))
            .await
    }

    pub(crate) async fn find_symbol_ok(&self, args: JsonObject) -> Value {
        self.find_symbol(args).await.expect("no error")
    }

    pub(crate) async fn symbol_info(&self, args: JsonObject) -> Result<Value, Value> {
        self.call_tool(CallToolRequestParams::new("symbol_info").with_arguments(args))
            .await
    }

    pub(crate) async fn symbol_info_ok(&self, args: JsonObject) -> Value {
        self.symbol_info(args).await.expect("no error")
    }
    pub(crate) async fn shutdown(mut self) {
        // take service service BEFORE potentially panicking
        let service = self.service.take().expect("not shut down yet");

        service.cancel().await.expect("shut down service");
    }
}

impl Drop for TestSetup {
    fn drop(&mut self) {
        if self.service.is_some() && !std::thread::panicking() {
            self.intercept_io_dir.disable_cleanup =
                Some(self.intercept_io_dir.disable_cleanup.unwrap_or_default());
            panic!("forgot to call shutdown");
        }
    }
}

fn normalize_paths(value: Value, fixtures_path: &str) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| normalize_paths(item, fixtures_path))
                .collect(),
        ),
        Value::Object(obj) => Value::Object(
            obj.into_iter()
                .map(|(key, value)| (key, normalize_paths(value, fixtures_path)))
                .collect(),
        ),
        Value::String(text) => Value::String(text.replace(fixtures_path, "/fixtures")),
        value => value,
    }
}

pub(crate) fn map<const N: usize>(m: [(&'static str, Value); N]) -> JsonObject {
    m.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

mod test {
    use super::*;

    #[tokio::test]
    #[should_panic(expected = "forgot to call shutdown")]
    async fn test_forgot_shutdown() {
        let _ = TestSetup::new().await;
    }

    #[tokio::test]
    #[should_panic(expected = "foo")]
    async fn test_forgot_shutdown_no_double_panic() {
        let mut setup = TestSetup::new().await;
        setup.intercept_io_dir.disable_cleanup = Some(false);
        panic!("foo")
    }
}
