Feature: Explicit step libraries

  Scenario: Account vocabulary
    Given the domain is empty

  Scenario: Filesystem vocabulary
    Given the domain is empty

  Scenario: Ambiguous vocabulary
    Given the domain is empty

  Scenario: Reversed ambiguous vocabulary
    Given the domain is empty

  Scenario: Async scoped vocabulary
    When the scoped account operation runs asynchronously

  Scenario: Harness scoped vocabulary
    Given the harness runs the scoped account vocabulary

  @allow_skipped
  Scenario: Scoped bypass vocabulary
    Given the scoped account scenario is skipped
    Then the scoped trailing step is bypassed
