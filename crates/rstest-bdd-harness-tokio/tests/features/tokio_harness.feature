Feature: Tokio harness integration

  Scenario: Tokio runtime is active during step execution
    Given the Tokio runtime is active
    When a Tokio handle is obtained
    Then the handle confirms current-thread execution

  Scenario: Tokio harness with attribute policy
    Given the Tokio runtime is active
    When a Tokio handle is obtained
    Then the handle confirms current-thread execution

  Scenario: Tokio harness with default attribute override
    Given a Tokio harness context is injected
    When the Tokio harness context handle is accessed
    Then the injected Tokio context proves harness ownership

  Scenario: Tokio attribute policy without harness
    Given the Tokio runtime is active
    When a Tokio handle is obtained
    Then the Tokio runtime remains available

  Scenario: Async step definitions execute under TokioHarness
    Given an async given step runs
    When an async when step runs
    Then the async steps completed
