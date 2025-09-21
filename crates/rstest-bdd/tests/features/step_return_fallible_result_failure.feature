@fallible @value
Feature: Fallible value step (failure)
  Scenario: Fallible value step failure
    Given base number is 1
    When a fallible increment fails
    Then the fallible result fails
