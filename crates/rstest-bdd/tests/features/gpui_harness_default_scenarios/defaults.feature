Feature: GPUI harness scenarios macro defaults

  Scenario: GPUI harness provides context through default attributes
    Given a GPUI test is running
    When I access the GPUI test context
    Then the GPUI test context is valid
