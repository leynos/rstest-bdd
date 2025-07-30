Feature: Step arguments
  Scenario: Deposit money
    Given I start with 100 dollars
    When I deposit 50 dollars
    Then my balance is 150 dollars
