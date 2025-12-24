Feature: Step return core::result::Result

  Scenario: core result unit and value
    Given base number is 1
    When a core fallible increment succeeds
    Then the fallible result is 2

