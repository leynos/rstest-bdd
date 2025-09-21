@fallible @unit
Feature: Fallible unit step
  Scenario: Fallible unit step success
    Given base number is 1
    When a fallible unit step succeeds
    Then the base number is unchanged
