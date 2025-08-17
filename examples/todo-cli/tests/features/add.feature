Feature: Add tasks
  Scenario: Add multiple tasks
    Given an empty to-do list
    When I add the following tasks
      | Buy milk |
      | Write tests |
    Then the list displays
      """
      1. [ ] Buy milk
      2. [ ] Write tests
      """

