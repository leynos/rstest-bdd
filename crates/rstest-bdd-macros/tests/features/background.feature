Feature: Background execution
  Background:
    Given a background step

  Scenario: first scenario
    When an action occurs
    Then a result is produced

  Scenario: second scenario
    When an action occurs
    Then a result is produced
