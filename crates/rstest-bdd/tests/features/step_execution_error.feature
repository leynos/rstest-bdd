Feature: Step execution error propagation

  Scenario: Handler error propagates through step loop
    Given a step that will fail
    Then this step should not execute
