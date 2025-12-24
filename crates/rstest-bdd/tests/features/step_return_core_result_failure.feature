Feature: Step return core::result::Result failure

  Scenario: core result failure
    Given base number is 1
    When a core fallible increment fails
    Then the fallible result fails
