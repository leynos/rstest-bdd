Feature: Skipping scenarios
  Scenario: disallowed skip
    Given a scenario will be skipped

  @allow_skipped
  Scenario: allowed skip
    Given a scenario will be skipped

  Scenario: skip without fail flag
    Given a scenario will be skipped

  Scenario: skip prevents trailing steps
    Given a scenario will be skipped
    Then a trailing step executes
