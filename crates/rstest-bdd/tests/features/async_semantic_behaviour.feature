Feature: Async semantic behaviour

  @allow_skipped
  Scenario: async skip propagation preserves metadata
    Given semantic async skip state is reset
    When semantic async skip is requested
    Then semantic async trailing step should never run

  Scenario Outline: async steps preserve declaration order
    Given semantic async order fixture starts with its creation marker
    When semantic async order fixture records "<item>"
    Then semantic async order fixture includes "<item>" in sequence

    Examples:
      | item  |
      | alpha |
      | beta  |

  Scenario: async returned fixtures reach the next step
    Given semantic async base value is 1
    When semantic async derived value is produced
    Then semantic async next step receives value 2

  Scenario: async RefCell fixtures survive cross-step borrows
    Given semantic async shared counter starts at 0
    When semantic async shared counter increments
    And semantic async shared counter increments
    Then semantic async shared counter equals 2

  Scenario: async failure surfaces scenario metadata
    Given semantic async failure state is reset
    When semantic async failing step runs
    Then semantic async failure trailing step should never run

  Scenario: cleanup probe completes successfully
    Given semantic cleanup probe is available

  Scenario: cleanup probe fails after setup
    Given semantic cleanup probe is available
    When semantic cleanup step fails
