Feature: Bulk-migration cookbook — first scenario

  Scenario: First scenario reuses the shared step library
    Given a fresh ledger
    When an entry of 10 is posted
    And an entry of 5 is posted
    Then the balance is 15
