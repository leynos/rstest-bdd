Feature: Unhinted step return alias payload

  Scenario: An alias result overrides the fixture with its payload
    Given base number is 1
    When an unhinted alias overrides the number
    Then the result is 2
