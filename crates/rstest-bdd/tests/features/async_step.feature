Feature: Async step infrastructure

  Scenario: Sync step normalized to async interface
    Given a synchronous step definition
    When the async wrapper is invoked
    Then it returns an immediately-ready future
