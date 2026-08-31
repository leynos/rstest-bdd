Feature: Explicit step libraries

  Scenario: Account vocabulary
    Given the domain is empty

  Scenario: Filesystem vocabulary
    Given the domain is empty

  Scenario: Ambiguous vocabulary
    Given the domain is empty

  @allow_skipped
  Scenario: Scoped bypass vocabulary
    Given the scoped account scenario is skipped
    Then the scoped trailing step is bypassed
