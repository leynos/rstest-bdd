Feature: Fallible GPUI scenario outline

  Scenario Outline: Fallible outline scenario
    Given a precondition
    When an action occurs
    Then a result is produced

    Examples:
      | case |
      | one  |
