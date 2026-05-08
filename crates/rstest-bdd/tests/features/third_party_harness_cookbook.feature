Feature: Third-party harness cookbook

  Scenario: Bevy-like harness mutates world context
    Given the cookbook world starts empty
    When the cookbook app spawns one entity
    Then the cookbook world contains one entity
