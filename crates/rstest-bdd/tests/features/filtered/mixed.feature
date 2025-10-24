Feature: Macro outline filtering
  Scenario Outline: outline example
    Given a precondition
    When an action occurs with <num>
    Then events are recorded
    And only fast examples run

    @fast
    Examples: fast
      | num |
      | 1   |
    @slow
    Examples: slow
      | num |
      | 2   |
