Feature: Step return rstest_bdd::StepResult

  Scenario: StepResult failure
    Given base number is 1
    When a StepResult increment fails

