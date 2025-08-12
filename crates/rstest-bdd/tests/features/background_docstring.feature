Feature: Background with docstring
  Background:
    Given the following message:
      """
      hello
      world
      """
  Scenario: uses background docstring
    Then the captured message equals:
      """
      hello
      world
      """
