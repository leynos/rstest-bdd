Feature: Gamma
  Scenario Outline: outline
    Given a precondition
    When an action occurs with <num>
    Then events are recorded

    Examples:
      | num |
      | 1   |
      | 2   |
