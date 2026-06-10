Feature: Harness-led attribute-policy defaults at runtime

  Scenario: Inferred GPUI policy provides the test context
    Given the inferred GPUI context is observed

  Scenario: Failing harness initialisation propagates
    Given a step that must never run
