Feature: Manual async scenario

  Scenario: Manual async scenario with tokio::test
    Given the manual async step runs
    When the manual async step continues
    Then the manual async step completes
