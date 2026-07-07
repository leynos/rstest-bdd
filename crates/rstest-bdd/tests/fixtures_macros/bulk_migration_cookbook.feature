Feature: Bulk-migration cookbook compile fixture

  Scenario: Shared step library compiles
    Given a fresh ledger
    When an entry of 10 is posted
    Then the balance is 10
