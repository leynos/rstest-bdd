Feature: Step return values with ambiguous fixtures
  Scenario: Ambiguous fixtures ignore override
    Given two competing fixtures
    When a step returns a competing value
    Then the fixtures remain unchanged
