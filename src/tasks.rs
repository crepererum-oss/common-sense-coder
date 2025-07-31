use std::panic::AssertUnwindSafe;

use anyhow::{Context, Error, Result};
use futures::{FutureExt, future::BoxFuture};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

#[derive(Debug, Default)]
pub(crate) struct TaskManager {
    tasks: JoinSet<(Result<()>, String)>,
    cancel: CancellationToken,
}

impl TaskManager {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn spawn<F, Fut, S>(&mut self, f: F, name: S)
    where
        F: FnOnce(CancellationToken) -> Fut,
        Fut: Future<Output = Result<()>> + Send + 'static,
        S: Into<String>,
    {
        let name: String = name.into();
        let future = f(self.cancel.clone());
        self.spawn_inner(Box::pin(future), name);
    }

    /// Non-generic version of `spawn`.
    #[inline(never)]
    fn spawn_inner(&mut self, future: BoxFuture<'static, Result<()>>, name: String) {
        self.tasks.spawn(async move {
            debug!(phase = "spawn", name = name.as_str(), "task");

            let res = AssertUnwindSafe(future).catch_unwind().await;

            let res = match res {
                Ok(Ok(())) => {
                    debug!(phase = "complete", name = name.as_str(), "task");
                    Ok(())
                }
                Ok(Err(e)) => {
                    warn!(phase = "error", name = name.as_str(), error = %e, "task");
                    Err(e)
                }
                Err(e) => {
                    let msg = e
                        .downcast_ref::<String>()
                        .map(|s| s.to_owned())
                        .or(e.downcast_ref::<&str>().map(|s| (*s).to_owned()));
                    warn!(
                        phase = "error",
                        name = name.as_str(),
                        msg = msg.as_deref(),
                        "task"
                    );
                    Err(Error::msg(msg.unwrap_or_else(|| "<unknown>".to_owned())).context("panic"))
                }
            };

            let res = res.with_context(|| format!("task {name}"));
            (res, name)
        });
    }

    pub(crate) async fn run(&mut self) -> Error {
        match self.tasks.join_next().await {
            None => {
                // not tasks => block forever
                futures::future::pending::<()>().await;
                unreachable!()
            }
            Some(Err(e)) => Error::new(e).context("join"),
            Some(Ok((Ok(()), name))) => Error::msg(format!("task '{name}' returned early")),
            Some(Ok((Err(e), _name))) => e,
        }
    }

    pub(crate) async fn shutdown(mut self) -> Result<()> {
        self.cancel.cancel();

        let mut res = Ok(());
        while let Some(task_res) = self.tasks.join_next().await {
            let task_res = match task_res {
                Ok((Ok(()), _name)) => Ok(()),
                Ok((Err(e), _name)) => Err(e),
                Err(e) => Err(Error::new(e).context("join")),
            };

            res = res.and(task_res);
        }

        res
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_panic_string_run() {
        let mut tasks = TaskManager::new();
        let s = String::from("hello");
        tasks.spawn(async move |_token| panic!("foo {s}"), "test");
        let err = tasks.run().await;
        assert_eq!(format!("{err:#}"), "task test: panic: foo hello");
    }

    #[tokio::test]
    async fn test_panic_string_shutdown() {
        let mut tasks = TaskManager::new();
        let s = String::from("hello");
        tasks.spawn(async move |_token| panic!("foo {s}"), "test");
        let err = tasks.shutdown().await.unwrap_err();
        assert_eq!(format!("{err:#}"), "task test: panic: foo hello");
    }

    #[tokio::test]
    async fn test_panic_str_run() {
        let mut tasks = TaskManager::new();
        tasks.spawn(async |_token| panic!("foo"), "test");
        let err = tasks.run().await;
        assert_eq!(format!("{err:#}"), "task test: panic: foo");
    }

    #[tokio::test]
    async fn test_panic_str_shutdown() {
        let mut tasks = TaskManager::new();
        tasks.spawn(async |_token| panic!("foo"), "test");
        let err = tasks.shutdown().await.unwrap_err();
        assert_eq!(format!("{err:#}"), "task test: panic: foo");
    }
}
