Feature: Multiple scenarios sharing one supplied adapter path

  Scenario: First scenario using the shared attribute policy
    Given a precondition
    When an action occurs
    Then an async result is produced

  Scenario: Second scenario using the shared attribute policy
    Given a precondition
    When an action occurs
    Then an async result is produced
