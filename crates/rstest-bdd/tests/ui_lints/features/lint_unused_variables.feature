Feature: Lint mitigation for unused scenario fixture parameters
  Scenario: Start harness with valid configuration
    Given a configured harness world
    When the harness starts
    Then startup succeeds
