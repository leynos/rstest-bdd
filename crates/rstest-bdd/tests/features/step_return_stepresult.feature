Feature: Step return rstest_bdd::StepResult

  Scenario: StepResult value
    Given base number is 1
    When a StepResult increment succeeds
    Then the fallible result is 2

