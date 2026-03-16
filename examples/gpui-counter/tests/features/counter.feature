Feature: Counter application with GPUI harness

  Scenario: Increment a counter and observe GPUI context
    Given a counter starting at 0
    When I increment the counter by 3
    And I record the GPUI dispatcher seed
    Then the counter value is 3
    And the recorded dispatcher seed is 0

  Scenario: Multiple increments and decrements
    Given a counter starting at 10
    When I decrement the counter by 4
    And I increment the counter by 7
    Then the counter value is 13
