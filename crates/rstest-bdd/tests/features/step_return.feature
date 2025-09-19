Feature: Step return values
  Scenario: When step output overrides fixture
    Given base number is 1
    When it is incremented
    Then the result is 2

  Scenario: Ambiguous fixtures ignore override
    Given two competing fixtures
    When a step returns a competing value
    Then the fixtures remain unchanged
