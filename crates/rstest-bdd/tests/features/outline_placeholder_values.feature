Feature: Placeholder-like Examples values

  Scenario Outline: Substituted values are kept verbatim
    When the step text holds "<first>" and "<second>"
    And the docstring holds:
      """
      <first> <second>
      """

    Examples:
      | first    | second  |
      | <second> | x       |
      | y        | <first> |
