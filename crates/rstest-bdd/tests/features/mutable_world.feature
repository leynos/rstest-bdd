Feature: Mutable fixture world
  Scenario: Steps mutate shared state
    Given the world starts at 2
    When the world increments
    Then the world equals 3
