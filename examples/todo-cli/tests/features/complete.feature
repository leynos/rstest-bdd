Feature: Complete tasks
  Scenario: Mark an item as done
    Given a todo list with Buy milk and Write tests
    When I complete Buy milk
    Then the task statuses should be
      | Buy milk | yes |
      | Write tests | no |
