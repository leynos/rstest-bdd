Feature: Empty Examples rows
  A row whose cells are all empty is a real case, and the rows after it
  still run their own substituted steps.

  Scenario Outline: Every row runs its own steps
    When the label "<label>" is journalled
    Then the journal holds the label "<label>"

    Examples:
      | label |
      | first |
      |       |
      | third |
