Feature: DocString handling
  Scenario: Capture docstring
    Given the following message:
      """
      hello
      world
      """
    Then the captured message equals:
      """
      hello
      world
      """
