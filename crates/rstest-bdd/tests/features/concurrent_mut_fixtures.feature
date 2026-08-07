Feature: Concurrent mutable fixture borrows

  Scenario: One step mutates two fixtures
    Given two counters are available
    When both counters are incremented in one step
    Then both counters reflect the increments

  Scenario: One step mutates harness context and world
    Given the harness world starts empty
    When the step mutates harness context and world together
    Then the harness context and world reflect the mutations
