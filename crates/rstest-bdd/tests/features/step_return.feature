Feature: Step return values
  Scenario: When step output overrides fixture
    Given base number is 1
    When it is incremented
    Then the result is 2
