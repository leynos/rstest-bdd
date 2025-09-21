@fallible @value
Feature: Fallible value step
  Scenario: Fallible value step success
    Given base number is 1
    When a fallible increment succeeds
    Then the fallible result is 2

  Scenario: Fallible value step failure
    Given base number is 1
    When a fallible increment fails
    Then the fallible result fails
