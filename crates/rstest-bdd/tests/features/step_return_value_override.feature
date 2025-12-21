Feature: Step return override for value hint

  Scenario: override value kind with inferred pattern
    Given base number is 1
    When value increment succeeds
    Then the result is 2
