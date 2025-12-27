Feature: Expression attribute syntax
  Scenario: Steps defined with expr syntax work correctly
    Given a counter initialised to zero
    When the counter is incremented by 5
    Then the counter equals 5
