Feature: Scenario Outline placeholder substitution

  Scenario Outline: Values are substituted in step text
    Given I have <start> items
    When I add <amount> more items
    Then I should have <total> items

    Examples:
      | start | amount | total |
      | 5     | 3      | 8     |
      | 10    | 5      | 15    |
