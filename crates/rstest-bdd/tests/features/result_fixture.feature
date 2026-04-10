Feature: Result-returning fixture injection

  Scenario: successful fixture initialization
    Given a world initialized from a Result fixture
    When the world is mutated
    Then the world reflects the mutation

  Scenario: failing fixture initialization
    Given a world initialized from a Result fixture

  Scenario: StepResult fixture success
    Given a world initialized from a StepResult fixture
    When the StepResult world is mutated
    Then the StepResult world reflects the mutation

  Scenario: StepResult fixture error
    Given a world initialized from a StepResult fixture
