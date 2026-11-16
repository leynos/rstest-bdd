Feature: Counting through the prelude

  Scenario: Counting up from zero
    Given a counter starting at 0
    When I add 2 to the counter
    Then the counter reads 2