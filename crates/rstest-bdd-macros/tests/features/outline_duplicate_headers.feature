Feature: Outline errors

  Scenario Outline: Duplicate headers
    Given a precondition
    When an action occurs
    Then a result is produced

    Examples:
        | num | num |
        | 1   | 1   |
