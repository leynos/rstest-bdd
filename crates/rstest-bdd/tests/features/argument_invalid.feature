Feature: Step arguments
  Scenario: Invalid deposit amount
    Given I start with 100 dollars
    When I deposit 4294967296 dollars
