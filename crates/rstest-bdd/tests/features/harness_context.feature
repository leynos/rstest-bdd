Feature: Harness context fixture injection

  Scenario: Step functions read and mutate harness context
    Given harness context starts with 7
    When harness context is incremented by 5
    Then harness context equals 12
