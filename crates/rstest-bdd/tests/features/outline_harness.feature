Feature: Outline with harness delegation

  Scenario Outline: Harness receives each example row
    Given a counted precondition for row <row>
    When an action occurs
    Then a result is produced

    Examples:
      | row |
      | 1   |
      | 2   |
