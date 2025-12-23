Feature: Step return core::result::Result

  Scenario: core result failure
    Given base number is 1
    When a core fallible increment fails

