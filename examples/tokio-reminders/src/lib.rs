//! Example reminder service for demonstrating Tokio harness integration with
//! `rstest-bdd`.
//!
//! The library keeps its asynchronous behaviour deliberately small: reminders
//! are scheduled onto Tokio's local task queue and become observable only after
//! an explicit flush. That makes the example deterministic while still showing
//! real async application flow rather than a harness smoke test.

use std::{cell::RefCell, future::Future, pin::Pin, rc::Rc};

use thiserror::Error;

type ReminderTask = Pin<Box<dyn Future<Output = ()>>>;

struct PendingReminder {
    recipient: String,
    task: ReminderTask,
}

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
#[derive(Clone, Default)]
pub struct ReminderService {
    delivered: Rc<RefCell<Vec<String>>>,
    pending: Rc<RefCell<Vec<PendingReminder>>>,
}

impl ReminderService {
    /// Creates an empty reminder service.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_reminders::ReminderService;
    ///
    /// let service = ReminderService::new();
    /// assert_eq!(service.pending_reminder_count(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedules a reminder for later delivery on Tokio's local task queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_reminders::ReminderService;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let runtime = tokio::runtime::Builder::new_current_thread()
    /// #     .enable_all()
    /// #     .build()?;
    /// # let local_set = tokio::task::LocalSet::new();
    /// # local_set.block_on(&runtime, async {
    /// let service = ReminderService::new();
    /// service.schedule_reminder("Ada");
    /// assert_eq!(service.pending_reminder_count(), 1);
    /// assert_eq!(service.pending_recipients(), vec!["Ada".to_string()]);
    /// # Ok::<(), tokio_reminders::ReminderServiceError>(())
    /// # })?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn schedule_reminder(&self, recipient: impl Into<String>) {
        let delivered = Rc::clone(&self.delivered);
        let recipient = recipient.into();
        let task_recipient = recipient.clone();

        // Store the future without spawning it yet
        let task: ReminderTask = Box::pin(async move {
            delivered
                .borrow_mut()
                .push(format!("Reminder sent to {task_recipient}"));
        });

        self.pending
            .borrow_mut()
            .push(PendingReminder { recipient, task });
    }

    /// Waits for all queued reminders to complete.
    ///
    /// # Panics
    ///
    /// This method panics if called outside a [`tokio::task::LocalSet`]. The
    /// implementation uses [`tokio::task::spawn_local`] to execute scheduled
    /// reminders, which requires an active `LocalSet` context. Ensure you call
    /// `flush` from within a `LocalSet` (e.g., via
    /// [`LocalSet::run_until`](tokio::task::LocalSet::run_until) or
    /// [`LocalSet::block_on`](tokio::task::LocalSet::block_on)).
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_reminders::ReminderService;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let runtime = tokio::runtime::Builder::new_current_thread()
    /// #     .enable_all()
    /// #     .build()?;
    /// # let local_set = tokio::task::LocalSet::new();
    /// # local_set.block_on(&runtime, async {
    /// let service = ReminderService::new();
    /// service.schedule_reminder("Ada");
    /// service.flush().await?;
    /// assert_eq!(service.pending_reminder_count(), 0);
    /// # Ok::<(), tokio_reminders::ReminderServiceError>(())
    /// # })?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn flush(&self) -> Result<(), ReminderServiceError> {
        let pending = std::mem::take(&mut *self.pending.borrow_mut());
        let mut first_error = None;

        for PendingReminder { task, .. } in pending {
            // Spawn the task and await it
            let handle = tokio::task::spawn_local(task);
            if let Err(source) = handle.await {
                if first_error.is_none() {
                    first_error = Some(ReminderServiceError::Join { source });
                }
            }
        }

        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(())
        }
    }

    /// Returns the delivered reminder messages in delivery order.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_reminders::ReminderService;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let runtime = tokio::runtime::Builder::new_current_thread()
    /// #     .enable_all()
    /// #     .build()?;
    /// # let local_set = tokio::task::LocalSet::new();
    /// # local_set.block_on(&runtime, async {
    /// let service = ReminderService::new();
    /// service.schedule_reminder("Ada");
    /// service.flush().await?;
    /// assert_eq!(service.delivered_reminders(), vec!["Reminder sent to Ada".to_string()]);
    /// # Ok::<(), tokio_reminders::ReminderServiceError>(())
    /// # })?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn delivered_reminders(&self) -> Vec<String> {
        self.delivered.borrow().clone()
    }

    /// Returns the number of reminders still waiting to be flushed.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_reminders::ReminderService;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let runtime = tokio::runtime::Builder::new_current_thread()
    /// #     .enable_all()
    /// #     .build()?;
    /// # let local_set = tokio::task::LocalSet::new();
    /// # local_set.block_on(&runtime, async {
    /// let service = ReminderService::new();
    /// assert_eq!(service.pending_reminder_count(), 0);
    /// service.schedule_reminder("Ada");
    /// assert_eq!(service.pending_reminder_count(), 1);
    /// # Ok::<(), tokio_reminders::ReminderServiceError>(())
    /// # })?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn pending_reminder_count(&self) -> usize {
        self.pending.borrow().len()
    }

    /// Returns the queued reminder recipients in scheduling order.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_reminders::ReminderService;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let runtime = tokio::runtime::Builder::new_current_thread()
    /// #     .enable_all()
    /// #     .build()?;
    /// # let local_set = tokio::task::LocalSet::new();
    /// # local_set.block_on(&runtime, async {
    /// let service = ReminderService::new();
    /// service.schedule_reminder("Ada");
    /// service.schedule_reminder("Grace");
    /// assert_eq!(service.pending_recipients(), vec!["Ada".to_string(), "Grace".to_string()]);
    /// # Ok::<(), tokio_reminders::ReminderServiceError>(())
    /// # })?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn pending_recipients(&self) -> Vec<String> {
        self.pending
            .borrow()
            .iter()
            .map(|pending| pending.recipient.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `ReminderService`.

    use std::rc::Rc;

    use super::{PendingReminder, ReminderService, ReminderTask};
    use rstest::{fixture, rstest};

    #[fixture]
    fn local_set() -> tokio::task::LocalSet {
        tokio::task::LocalSet::new()
    }

    #[fixture]
    fn service() -> ReminderService {
        ReminderService::new()
    }

    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn starts_with_no_pending_or_delivered_reminders(
        local_set: tokio::task::LocalSet,
        service: ReminderService,
    ) {
        local_set
            .run_until(async move {
                assert_eq!(service.pending_reminder_count(), 0);
                assert!(service.pending_recipients().is_empty());
                assert!(service.delivered_reminders().is_empty());
            })
            .await;
    }

    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn flush_delivers_scheduled_reminders_in_order(
        local_set: tokio::task::LocalSet,
        service: ReminderService,
    ) {
        local_set
            .run_until(async move {
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

    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn flush_only_waits_for_the_current_batch(
        local_set: tokio::task::LocalSet,
        service: ReminderService,
    ) {
        local_set
            .run_until(async move {
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

    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn flush_awaits_every_pending_task_before_returning_first_error(
        local_set: tokio::task::LocalSet,
        service: ReminderService,
    ) {
        local_set
            .run_until(async move {
                let delivered = Rc::clone(&service.delivered);
                let failing_task: ReminderTask = Box::pin(async move {
                    panic!("failing reminder task");
                });
                let succeeding_task: ReminderTask = Box::pin(async move {
                    delivered
                        .borrow_mut()
                        .push("Reminder sent to Grace".to_string());
                });

                service.pending.borrow_mut().extend([
                    PendingReminder {
                        recipient: "Ada".to_string(),
                        task: failing_task,
                    },
                    PendingReminder {
                        recipient: "Grace".to_string(),
                        task: succeeding_task,
                    },
                ]);

                let result = service.flush().await;
                assert!(result.is_err(), "flush should report the first join error");
                assert_eq!(
                    service.delivered_reminders(),
                    vec!["Reminder sent to Grace".to_string()]
                );
                assert_eq!(service.pending_reminder_count(), 0);
                assert!(service.pending_recipients().is_empty());
            })
            .await;
    }

    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn scheduled_reminders_do_not_deliver_until_flush(
        local_set: tokio::task::LocalSet,
        service: ReminderService,
    ) {
        local_set
            .run_until(async move {
                service.schedule_reminder("Ada");

                // Yield to give any spawned tasks a chance to run
                tokio::task::yield_now().await;

                // Delivered should still be empty because flush hasn't been called
                assert!(
                    service.delivered_reminders().is_empty(),
                    "delivered_reminders should be empty before flush"
                );
                assert_eq!(
                    service.pending_reminder_count(),
                    1,
                    "pending count should be 1 before flush"
                );

                // Now flush and verify delivery
                service.flush().await.expect("flush should succeed");
                assert_eq!(
                    service.delivered_reminders(),
                    vec!["Reminder sent to Ada".to_string()],
                    "delivered_reminders should contain Ada after flush"
                );
                assert_eq!(
                    service.pending_reminder_count(),
                    0,
                    "pending count should be 0 after flush"
                );
            })
            .await;
    }
}
