Feature: Doc string parameter order
  Scenario: docstring precedes value
    Given message then value 5:
      """
      alpha
      """

  Scenario: value precedes docstring in function signature
    Given value then message 5:
      """
      alpha
      """
