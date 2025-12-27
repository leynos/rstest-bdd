Feature: Invalid placeholder reference

  Scenario Outline: Uses undefined placeholder
    Given step with <undefined>

    Examples:
      | valid |
      | value |
