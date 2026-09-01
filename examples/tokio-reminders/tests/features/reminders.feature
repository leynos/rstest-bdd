Feature: Tokio reminder delivery

  Scenario: Scheduling a reminder queues it for later delivery
    Given a reminder service
    When I schedule a reminder for Ada
    Then the pending reminder count is 1
    And the pending recipients are
      | Ada |
    And no reminders have been delivered yet

  Scenario: Scheduling multiple reminders preserves queue order
    Given a reminder service
    When I schedule a reminder for Ada
    And I schedule a reminder for Grace
    Then the pending reminder count is 2
    And the pending recipients are
      | Ada |
      | Grace |
    And no reminders have been delivered yet

  Scenario: A step can reach the Tokio harness context through the marker
    Given a reminder service
    When I reach the harness context through the marker
    Then no reminders have been delivered yet
