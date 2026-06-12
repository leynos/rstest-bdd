Feature: Harness-led attribute-policy defaults at runtime

  Scenario: Inferred GPUI policy provides the test context
    Given the inferred GPUI context is observed
    When the inferred GPUI context is mutated
    Then the inferred GPUI context remains available

  Scenario: Failing harness initialisation propagates
    Given a step that must never run
