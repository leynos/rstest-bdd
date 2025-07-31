Feature: Outline errors

  Scenario Outline: Empty examples
    Given a precondition
    When an action occurs
    Then a result is produced

    Examples:
        | num |
