Feature: Step return values
  Scenario: When step output overrides fixture
    Given base number is 1
    When it is incremented
    Then the result is 2

  Scenario: Ambiguous fixtures ignore override
    Given two competing fixtures
    When a step returns a competing value
    Then the fixtures remain unchanged

  Scenario: Fallible unit step success
    Given base number is 1
    When a fallible unit step succeeds
    Then the base number is unchanged

  Scenario: Fallible result step success
    Given base number is 1
    When a fallible increment succeeds
    Then the fallible result is 2

