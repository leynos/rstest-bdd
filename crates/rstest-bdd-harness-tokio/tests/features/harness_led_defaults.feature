Feature: Harness-led attribute-policy defaults at runtime

  Scenario: Inferred Tokio policy provides the runtime
    Given the inferred Tokio runtime is active
    When a local task is spawned under the inferred policy
    Then the inferred runtime flavour is current thread

  Scenario: Failing harness initialization propagates
    Given a step that must never run

  Scenario: Attribute policy alone does not provide a LocalSet
    Given a step that spawns a local task

  Scenario: An explicit default-policy downgrade stays silent
    Given a scenario bound with the Tokio harness and the default attribute policy
    When the scenario body checks for a Tokio runtime
    Then a Tokio runtime handle is available

  Scenario: A third-party harness without an attribute policy stays silent
    Given a scenario bound with a local harness and no attribute policy
    When the scenario body runs
    Then the local harness recorded one run

  Scenario: Removing a conflicting attribute override restores the harness default
    Given a scenario bound with the Tokio harness and no attribute policy
    When the scenario body checks for a Tokio runtime
    Then a Tokio runtime handle is available
