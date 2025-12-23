Feature: Step return rstest_bdd::StepResult failure

  Scenario: StepResult failure
    Given base number is 1
    When a StepResult increment fails
    Then the fallible result fails
