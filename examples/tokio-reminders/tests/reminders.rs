//! BDD acceptance tests for the Tokio reminders example.
//!
//! These scenarios demonstrate that `TokioHarness` can drive immediate-ready
//! `async fn` step definitions while the example queues local Tokio work.

use rstest::fixture;
use rstest_bdd_harness_tokio::TokioTestContext;
use rstest_bdd_macros::{given, scenario, then, when};
use tokio_reminders::ReminderService;

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn service() -> ReminderService {
    let service = ReminderService::new();
    std::hint::black_box(&service);
    service
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

#[when("I dispatch delivery on the harness runtime")]
async fn dispatch_delivery(
    service: &ReminderService,
    #[harness_context] context: &TokioTestContext,
) {
    // Prove that `#[harness_context]` delivered the harness-provided runtime
    // handle by checking it is the handle of the runtime currently active on
    // this thread. This mirrors the `runtime_flavor`/`Handle::current` checks
    // used by the harness's own integration tests and, being synchronous,
    // keeps the step single-poll under the harness wrapper.
    assert_eq!(
        context.handle().id(),
        tokio::runtime::Handle::current().id(),
        "marker-reached harness context must expose the active runtime handle"
    );

    // The step runs on the harness's LocalSet, so the queued delivery tasks can
    // be driven synchronously from here without an `.await` (which the harness
    // wrapper may only poll once).
    service.deliver_all();
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

#[then("the delivered reminders are")]
async fn the_delivered_reminders_are(
    service: &ReminderService,
    #[datatable] rows: Vec<Vec<String>>,
) {
    let actual = service.delivered_reminders();
    let expected = rows
        .into_iter()
        .map(|mut row| {
            assert_eq!(
                row.len(),
                1,
                "datatable rows should contain exactly one delivered-reminder column: {row:?}"
            );
            row.swap_remove(0)
        })
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[scenario(
    path = "tests/features/reminders.feature",
    name = "Scheduling a reminder queues it for later delivery",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn queues_a_scheduled_reminder(#[from(service)] _: ReminderService) {}

#[scenario(
    path = "tests/features/reminders.feature",
    name = "Scheduling multiple reminders preserves queue order",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn preserves_queue_order(#[from(service)] _: ReminderService) {}

#[scenario(
    path = "tests/features/reminders.feature",
    name = "A step can reach the Tokio harness context through the marker",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn step_can_reach_harness_context_through_the_marker(#[from(service)] _: ReminderService) {}
