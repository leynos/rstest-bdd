Feature: Missing fixture error propagation

  Scenario: Missing fixture causes panic
    Given a registered step
    When a step needs fixture
    Then this step should not execute
