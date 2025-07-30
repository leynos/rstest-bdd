Feature: Outline errors

  Scenario Outline: Missing column
    Given a precondition
    When an action occurs
    Then a result is produced

    Examples:
      | num | text |
      | 1   |
