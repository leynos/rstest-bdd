Feature: Lint mitigation for fixture borrow guards in step wrappers
  Scenario: Steps borrow fixtures by shared reference, mutable reference and value
    Given a shared ledger reference
    When a mutable ledger reference records an entry
    And an async mutable ledger reference records an entry
    Then the owned ledger copy holds two entries
    And the async shared ledger reference holds two entries
