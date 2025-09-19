Feature: Fallible value step
  Scenario: Fallible value step success
    Given base number is 1
    When a fallible increment succeeds
    Then the fallible result is 2
