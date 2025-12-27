Feature: Fixture injection via scenarios! macro

  Scenario: Fixture is injected into step context
    Given a counter fixture
    When the counter is incremented
    Then the counter equals 1
