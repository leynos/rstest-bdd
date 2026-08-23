//! Ownership of the background task used for workspace preparation.

use tokio::task::JoinHandle;

use super::ServerState;

/// A task owned by a language-server instance while workspace work is pending.
#[derive(Default)]
pub(super) struct WorkspaceTask(Option<JoinHandle<()>>);

impl WorkspaceTask {
    /// Return whether an owned task is still retained.
    pub(super) fn has_retained_task(&self) -> bool {
        self.0.is_some()
    }

    /// Abort and discard the currently owned task.
    pub(super) fn abort(&mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
        }
    }
}

impl ServerState {
    /// Replace the owned workspace task, aborting superseded work first.
    pub fn replace_workspace_task(&mut self, task: JoinHandle<()>) {
        self.workspace_task.abort();
        self.workspace_task.0 = Some(task);
    }

    /// Drop the owned task handle after its result has been applied.
    pub(crate) fn clear_workspace_task(&mut self) {
        self.workspace_task.0 = None;
    }

    /// Take the task so shutdown can cancel and await it.
    pub fn take_workspace_task(&mut self) -> Option<JoinHandle<()>> {
        self.workspace_task.0.take()
    }
}

#[cfg(test)]
mod tests {
    //! Tests for cancellation of server-owned workspace work.

    use std::future::pending;

    use tokio::sync::oneshot;

    use super::*;
    use crate::config::ServerConfig;

    struct CancellationSignal(Option<oneshot::Sender<()>>);

    impl Drop for CancellationSignal {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }

    #[tokio::test]
    async fn retry_aborts_the_superseded_workspace_task() {
        let (cancelled_sender, cancelled) = oneshot::channel();
        let (started_sender, started) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _signal = CancellationSignal(Some(cancelled_sender));
            if started_sender.send(()).is_err() {
                return;
            }
            pending::<()>().await;
        });
        let mut state = ServerState::new(ServerConfig::default());
        state.replace_workspace_task(task);
        assert!(started.await.is_ok(), "workspace task should start");

        state.begin_workspace_initialization(Vec::new(), true);

        assert!(
            cancelled.await.is_ok(),
            "retry should cancel workspace work"
        );
    }
}
