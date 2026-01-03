Feature: Async scenario execution

  @async
  Scenario: Async steps execute sequentially
    Given an async counter is initialised to 0
    When the async counter is incremented
    And the async counter is incremented
    Then the async counter value is 2

  @async @allow_skipped
  Scenario: Skip works in async context
    Given an async counter is initialised to 0
    When the async step requests skip
    And the async counter is incremented after skip
    Then the async counter value is 0
