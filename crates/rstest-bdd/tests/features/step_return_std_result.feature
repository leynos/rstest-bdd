Feature: Step return std::result::Result

  Scenario: std result unit and value
    Given base number is 1
    When a std fallible unit step succeeds
    When a fallible increment succeeds
    Then the fallible result is 2

