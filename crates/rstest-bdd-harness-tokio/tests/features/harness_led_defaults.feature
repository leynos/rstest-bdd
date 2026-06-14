Feature: Harness-led attribute-policy defaults at runtime

  Scenario: Inferred Tokio policy provides the runtime
    Given the inferred Tokio runtime is active
    When a local task is spawned under the inferred policy
    Then the inferred runtime flavour is current thread

  Scenario: Spawned local task can be aborted cleanly
    Given the inferred Tokio runtime is active
    When a long-running local task is spawned and then aborted
    Then the task reports cancellation

  Scenario: Failing harness initialisation propagates
    Given a step that must never run

  Scenario: Attribute policy alone does not provide a LocalSet
    Given a step that spawns a local task
