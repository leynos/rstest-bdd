Feature: GPUI harness integration

  Scenario: GPUI harness injects TestAppContext
    Given a GPUI test context is injected
    When the GPUI test context is accessed mutably
    Then the same GPUI context remains available

  Scenario: GPUI harness with GPUI attribute policy
    Given a GPUI test context is injected
    When the GPUI test context is accessed mutably
    Then the same GPUI context remains available

  Scenario: GPUI attribute policy runs without harness
    Given a plain GPUI policy scenario runs
    Then the plain GPUI policy scenario completed
