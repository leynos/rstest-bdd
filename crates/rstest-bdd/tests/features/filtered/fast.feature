Feature: Macro filtered scenarios
  @fast
  Scenario: fast macro scenario
    Given a precondition
    When an action occurs
    Then events are recorded

  @slow
  Scenario: slow macro scenario
    Given a precondition
    When a slow action occurs
    Then slow events are recorded
