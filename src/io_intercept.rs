//! Tooling to intercept IO streams to/from external sources for debugging.
use std::{
    io::Error,
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::Context as _;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf},
    sync::mpsc::UnboundedSender,
};

use crate::TaskManager;

/// Dyn-typed [`AsyncWrite`].
pub(crate) type BoxWrite = Pin<Box<dyn AsyncWrite + Send>>;

/// Dyn-typed [`AsyncRead`]
pub(crate) type BoxRead = Pin<Box<dyn AsyncRead + Send>>;

/// Dumps [`AsyncWrite`] data to a file.
pub(crate) struct WriteFork {
    inner: BoxWrite,
    tx: UnboundedSender<Message>,
}

impl WriteFork {
    /// Create new fork in given directory path.
    ///
    /// The directory MUST exist.
    ///
    /// `what` is used for logging but also as a filename.
    pub(crate) async fn new(
        inner: BoxWrite,
        directory: &Path,
        what: &'static str,
        tasks: &mut TaskManager,
    ) -> anyhow::Result<Self> {
        let tx = spawn_writer(directory, what, tasks).await?;
        Ok(Self { inner, tx })
    }
}

impl AsyncWrite for WriteFork {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        self.inner.as_mut().poll_write(cx, buf).map_ok(|written| {
            self.tx.send(Message::Data(buf[..written].to_owned())).ok();
            written
        })
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        self.inner.as_mut().poll_flush(cx).map_ok(|()| {
            self.tx.send(Message::Flush).ok();
        })
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        self.inner.as_mut().poll_shutdown(cx).map_ok(|()| {
            self.tx.send(Message::Shutdown).ok();
        })
    }
}

/// Dumps [`AsyncRead`] data to a file.
pub(crate) struct ReadFork {
    inner: BoxRead,
    tx: UnboundedSender<Message>,
}

impl ReadFork {
    /// Create new fork in given directory path.
    ///
    /// The directory MUST exist.
    ///
    /// `what` is used for logging but also as a filename.
    pub(crate) async fn new(
        inner: BoxRead,
        directory: &Path,
        what: &'static str,
        tasks: &mut TaskManager,
    ) -> anyhow::Result<Self> {
        let tx = spawn_writer(directory, what, tasks).await?;
        Ok(Self { inner, tx })
    }
}

impl AsyncRead for ReadFork {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let pos_pre = buf.capacity() - buf.remaining();
        self.inner.as_mut().poll_read(cx, buf).map_ok(|()| {
            self.tx
                .send(Message::Data(buf.filled()[pos_pre..].to_owned()))
                .ok();
        })
    }
}

/// Message from fork to background writer task.
///
/// Messages are sent AFTER they succeed on the original [`AsyncWrite`]/[`AsyncRead`].
#[derive(Debug)]
enum Message {
    /// Write data.
    ///
    /// This comes from [`AsyncWrite::poll_write`] or [`AsyncRead::poll_read`].
    Data(Vec<u8>),

    /// Flush buffers.
    ///
    /// This comes from [`AsyncWrite::poll_flush`].
    Flush,

    /// Shut down file.
    ///
    /// This comes from [`AsyncWrite::poll_shutdown`].
    Shutdown,
}

/// Spawn background writer task.
///
/// The task will finish after sending [`Message::Shutdown`] or after all [senders](UnboundedSender) are dropped.
async fn spawn_writer(
    directory: &Path,
    what: &'static str,
    tasks: &mut TaskManager,
) -> anyhow::Result<UnboundedSender<Message>> {
    let file = tokio::fs::File::options()
        .append(true)
        .create(true)
        .open(directory.join(what))
        .await
        .with_context(|| format!("open {what} interception file"))?;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    tasks.spawn(
        async move |cancel| {
            let mut file = file;
            let mut rx = rx;

            while let Some(msg) = tokio::select! {
                biased;
                next = rx.recv() => next,
                _ = cancel.cancelled() => None,
            } {
                match msg {
                    Message::Data(data) => {
                        file.write_all(&data).await.context("write data")?;
                    }
                    Message::Flush => {
                        file.flush().await.context("flush file")?;
                    }
                    Message::Shutdown => {
                        break;
                    }
                }
            }

            file.flush().await.context("flush file")?;
            file.shutdown().await.context("shut down file")?;
            Ok(())
        },
        what,
    );

    Ok(tx)
}
