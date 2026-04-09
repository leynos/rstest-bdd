Feature: GPUI harness with attribute policy via scenarios macro

  Scenario: GPUI harness provides context through scenarios macro
    Given a GPUI test is running
    When I access the GPUI test context
    Then the GPUI test context is valid
