Feature: Step return override for type aliases

  Scenario: override alias result kind succeeds
    Given base number is 1
    When alias increment succeeds
    Then the result is 2
