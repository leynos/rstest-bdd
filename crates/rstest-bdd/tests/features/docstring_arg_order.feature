Feature: Doc string parameter order
  Scenario: docstring precedes value
    Given message then value 5:
      """
      alpha
      """
