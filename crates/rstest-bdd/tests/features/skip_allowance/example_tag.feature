Feature: Example tags do not allow skipping
  Scenario Outline: example tag ignored
    Given a scenario will be skipped

    @allow_skipped
    Examples:
      | case |
      | row  |
