Feature: Bulk-migration cookbook — second scenario

  Scenario: Second scenario reuses the shared step library
    Given a fresh ledger
    When an entry of 25 is posted
    Then the balance is 25
