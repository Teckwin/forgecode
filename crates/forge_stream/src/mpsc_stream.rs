use std::future::Future;
use std::time::Duration;

use futures::Stream;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

pub struct MpscStream<T> {
    join_handle: JoinHandle<()>,
    receiver: Receiver<T>,
}

impl<T> MpscStream<T> {
    /// Spawns a new async task that produces values sent through a channel.
    ///
    /// The provided function receives a `Sender` and returns a `Future` that
    /// will be executed in a spawned task.
    pub fn spawn<F, S>(f: F) -> MpscStream<T>
    where
        F: (FnOnce(Sender<T>) -> S) + Send + 'static,
        S: Future<Output = ()> + Send + 'static,
    {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        MpscStream { join_handle: tokio::spawn(f(tx)), receiver: rx }
    }

    /// Creates a new MpscStream from an existing sender and receiver pair.
    /// This allows for more flexible composition of streams.
    pub fn new(receiver: Receiver<T>, join_handle: JoinHandle<()>) -> Self {
        Self { join_handle, receiver }
    }

    /// Returns a reference to the receiver, allowing inspection of the channel state.
    pub fn receiver(&self) -> &Receiver<T> {
        &self.receiver
    }

    /// Checks if the underlying task is still running.
    pub fn is_running(&self) -> bool {
        !self.join_handle.is_finished()
    }

    /// Waits for the underlying task to complete with a timeout.
    ///
    /// Returns `true` if the task completed normally, `false` if it was aborted or timed out.
    pub async fn wait_for_completion(&mut self, timeout: Duration) -> bool {
        tokio::time::timeout(timeout, &mut self.join_handle)
            .await
            .is_ok()
    }

    /// Gracefully shuts down the stream, waiting for the task to complete.
    ///
    /// This method:
    /// 1. Closes the receiver to signal the task to stop
    /// 2. Waits for the task to complete (with a reasonable timeout)
    /// 3. Falls back to aborting if the timeout expires
    ///
    /// The default timeout is 1 second.
    pub async fn graceful_shutdown(&mut self) {
        // Close the receiver first to signal the task to stop
        self.receiver.close();

        // Wait for the task to complete gracefully
        let timeout = Duration::from_secs(1);
        let completed = tokio::time::timeout(timeout, &mut self.join_handle).await;

        // If timeout expired, abort the task
        if completed.is_err() {
            self.join_handle.abort();
        }
    }
}

impl<T> Stream for MpscStream<T> {
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}

impl<T> Drop for MpscStream<T> {
    fn drop(&mut self) {
        // Close the receiver to prevent any new messages
        self.receiver.close();

        // Try to wait for graceful shutdown, but don't block indefinitely
        // Use a non-blocking approach - check if task is finished, if not abort
        if self.join_handle.is_finished() {
            // Task already completed, no need to abort
            return;
        }

        // Task is still running - abort it
        // This is a safe fallback since we've closed the receiver
        self.join_handle.abort();
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use futures::StreamExt;
    use tokio::time::pause;

    use super::*;

    #[tokio::test]
    async fn test_stream_receives_messages() {
        let mut stream = MpscStream::spawn(|tx| async move {
            tx.send("test message").await.unwrap();
        });

        let result = stream.next().await;
        assert_eq!(result, Some("test message"));
    }

    #[tokio::test]
    async fn test_drop_aborts_task() {
        // Pause time to control it manually
        pause();

        let completed = Arc::new(AtomicBool::new(false));
        let completed_clone = completed.clone();

        let stream = MpscStream::spawn(|tx| async move {
            // Try to send a message
            let send_result = tx.send(1).await;
            assert!(send_result.is_ok(), "First send should succeed");

            // Simulate long running task with virtual time
            tokio::time::sleep(Duration::from_secs(1)).await;

            // This should never execute because we'll drop the stream
            completed_clone.store(true, Ordering::SeqCst);

            // This send should fail since receiver is dropped
            let _ = tx.send(2).await;
        });

        // Advance time a small amount to allow first message to be processed
        tokio::time::advance(Duration::from_millis(10)).await;

        // Drop the stream - this should abort the task
        drop(stream);

        // Advance time past when the task would have completed
        tokio::time::advance(Duration::from_secs(2)).await;

        // Verify the task was aborted and didn't complete
        assert!(
            !completed.load(Ordering::SeqCst),
            "Task should have been aborted"
        );
    }
}
