//! Example reminder service for demonstrating Tokio harness integration with
//! `rstest-bdd`.
//!
//! The library keeps its asynchronous behaviour deliberately small: reminders
//! are scheduled onto Tokio's local task queue and become observable only after
//! an explicit flush. That makes the example deterministic while still showing
//! real async application flow rather than a harness smoke test.

use std::{cell::RefCell, rc::Rc};

use thiserror::Error;
use tokio::task::JoinHandle;

/// Error produced when the reminder queue cannot finish scheduled work.
#[derive(Debug, Error)]
pub enum ReminderServiceError {
    /// A scheduled reminder task failed before it could complete.
    #[error("failed to join a scheduled reminder task: {source}")]
    Join {
        /// Tokio's join error from the failed task.
        #[source]
        source: tokio::task::JoinError,
    },
}

/// Queues reminder deliveries onto Tokio's local task set.
///
/// The service is intentionally `!Send` and `!Sync`: it uses `Rc<RefCell<_>>`
/// so the example can demonstrate `tokio::task::spawn_local` under
/// `TokioHarness`'s `LocalSet`.
///
/// # Examples
///
/// ```
/// use tokio_reminders::ReminderService;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let runtime = tokio::runtime::Builder::new_current_thread()
///     .enable_all()
///     .build()?;
/// let local_set = tokio::task::LocalSet::new();
///
/// local_set.block_on(&runtime, async {
///     let service = ReminderService::new();
///     service.schedule_reminder("Ada");
///     service.flush().await?;
///     assert_eq!(
///         service.delivered_reminders(),
///         vec!["Reminder sent to Ada".to_string()]
///     );
///     Ok::<(), tokio_reminders::ReminderServiceError>(())
/// })?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default)]
pub struct ReminderService {
    delivered: Rc<RefCell<Vec<String>>>,
    pending: Rc<RefCell<Vec<JoinHandle<()>>>>,
    pending_recipients: Rc<RefCell<Vec<String>>>,
}

impl ReminderService {
    /// Creates an empty reminder service.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedules a reminder for later delivery on Tokio's local task queue.
    pub fn schedule_reminder(&self, recipient: impl Into<String>) {
        let delivered = Rc::clone(&self.delivered);
        let recipient = recipient.into();
        self.pending_recipients.borrow_mut().push(recipient.clone());
        let handle = tokio::task::spawn_local(async move {
            delivered
                .borrow_mut()
                .push(format!("Reminder sent to {recipient}"));
        });

        self.pending.borrow_mut().push(handle);
    }

    /// Waits for all queued reminders to complete.
    pub async fn flush(&self) -> Result<(), ReminderServiceError> {
        let pending = self.pending.take();
        for handle in pending {
            handle
                .await
                .map_err(|source| ReminderServiceError::Join { source })?;
        }
        self.pending_recipients.borrow_mut().clear();

        Ok(())
    }

    /// Returns the delivered reminder messages in delivery order.
    #[must_use]
    pub fn delivered_reminders(&self) -> Vec<String> {
        self.delivered.borrow().clone()
    }

    /// Returns the number of reminders still waiting to be flushed.
    #[must_use]
    pub fn pending_reminder_count(&self) -> usize {
        self.pending.borrow().len()
    }

    /// Returns the queued reminder recipients in scheduling order.
    #[must_use]
    pub fn pending_recipients(&self) -> Vec<String> {
        self.pending_recipients.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `ReminderService`.

    use super::ReminderService;

    async fn run_local_test(test_fn: impl std::future::Future<Output = ()>) {
        let local_set = tokio::task::LocalSet::new();
        local_set.run_until(test_fn).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn starts_with_no_pending_or_delivered_reminders() {
        run_local_test(async {
            let service = ReminderService::new();

            assert_eq!(service.pending_reminder_count(), 0);
            assert!(service.pending_recipients().is_empty());
            assert!(service.delivered_reminders().is_empty());
        })
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn flush_delivers_scheduled_reminders_in_order() {
        run_local_test(async {
            let service = ReminderService::new();
            service.schedule_reminder("Ada");
            service.schedule_reminder("Grace");

            assert_eq!(service.pending_reminder_count(), 2);
            assert_eq!(
                service.pending_recipients(),
                vec!["Ada".to_string(), "Grace".to_string()]
            );

            let result = service.flush().await;
            assert!(
                result.is_ok(),
                "flush should complete scheduled reminders: {result:?}"
            );
            assert_eq!(
                service.delivered_reminders(),
                vec![
                    "Reminder sent to Ada".to_string(),
                    "Reminder sent to Grace".to_string(),
                ]
            );
            assert_eq!(service.pending_reminder_count(), 0);
            assert!(service.pending_recipients().is_empty());
        })
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn flush_only_waits_for_the_current_batch() {
        run_local_test(async {
            let service = ReminderService::new();
            service.schedule_reminder("Ada");

            let first = service.flush().await;
            assert!(first.is_ok(), "first flush should succeed: {first:?}");
            assert_eq!(
                service.delivered_reminders(),
                vec!["Reminder sent to Ada".to_string()]
            );
            assert!(service.pending_recipients().is_empty());

            service.schedule_reminder("Linus");
            assert_eq!(service.pending_recipients(), vec!["Linus".to_string()]);

            let second = service.flush().await;
            assert!(second.is_ok(), "second flush should succeed: {second:?}");
            assert_eq!(
                service.delivered_reminders(),
                vec![
                    "Reminder sent to Ada".to_string(),
                    "Reminder sent to Linus".to_string(),
                ]
            );
        })
        .await;
    }
}
