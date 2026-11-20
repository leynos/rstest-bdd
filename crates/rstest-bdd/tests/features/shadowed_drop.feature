Feature: Fixture guards with a shadowed drop in scope

  Scenario: Two steps borrow the same fixture mutably
    When the counter is incremented
    And the counter is incremented
    Then the counter reads 2
