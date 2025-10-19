@global
Feature: Tagged filters
  @fast
  Scenario: fast scenario
    Given a precondition
    When an action occurs
    Then a result is produced

  @slow
  Scenario: slow scenario
    Given a precondition
    When an action occurs
    Then a result is produced

  Scenario Outline: parameterised scenario
    Given a precondition
    When an action occurs
    Then a result is produced

    @outline_shared
    Examples: default
      | num |
      | 1   |
    @fast
    Examples: fast_only
      | num |
      | 2   |
