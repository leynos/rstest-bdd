//! BDD acceptance tests for the Tokio reminders example.
//!
//! These scenarios demonstrate that `TokioHarness` can drive immediate-ready
//! `async fn` step definitions while the example queues local Tokio work.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tokio_reminders::ReminderService;

#[fixture]
fn service() -> ReminderService {
    ReminderService::new()
}

#[given("a reminder service")]
fn a_reminder_service(service: &ReminderService) {
    assert_eq!(service.pending_reminder_count(), 0);
    assert!(service.pending_recipients().is_empty());
    assert!(service.delivered_reminders().is_empty());
}

#[when("I schedule a reminder for {recipient}")]
async fn schedule_a_reminder(service: &ReminderService, recipient: String) {
    service.schedule_reminder(recipient);
}

#[then("the pending reminder count is {expected:usize}")]
async fn the_pending_reminder_count_is(service: &ReminderService, expected: usize) {
    assert_eq!(service.pending_reminder_count(), expected);
}

#[then("the pending recipients are")]
async fn the_pending_recipients_are(
    service: &ReminderService,
    #[datatable] rows: Vec<Vec<String>>,
) {
    let actual = service.pending_recipients();
    let expected = rows
        .into_iter()
        .map(|mut row| {
            assert_eq!(
                row.len(),
                1,
                "datatable rows should contain exactly one recipient column: {row:?}"
            );
            row.swap_remove(0)
        })
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[then("no reminders have been delivered yet")]
async fn no_reminders_have_been_delivered_yet(service: &ReminderService) {
    assert!(service.delivered_reminders().is_empty());
}

#[scenario(
    path = "tests/features/reminders.feature",
    name = "Scheduling a reminder queues it for later delivery",
    harness = rstest_bdd_harness_tokio::TokioHarness,
    attributes = rstest_bdd_harness_tokio::TokioAttributePolicy,
)]
fn queues_a_scheduled_reminder(#[from(service)] _: ReminderService) {}

#[scenario(
    path = "tests/features/reminders.feature",
    name = "Scheduling multiple reminders preserves queue order",
    harness = rstest_bdd_harness_tokio::TokioHarness,
    attributes = rstest_bdd_harness_tokio::TokioAttributePolicy,
)]
fn preserves_queue_order(#[from(service)] _: ReminderService) {}
