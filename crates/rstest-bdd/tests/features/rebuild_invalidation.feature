Feature: Feature-file rebuild invalidation

  Scenario: A bound feature file is a tracked build dependency
    Given a scenario crate bound to a feature file
    When the crate is compiled
    Then the dep-info for the test binary lists the feature file

  Scenario: Editing only a feature file forces a rebuild and a fresh failure
    Given a scenario crate bound to a feature file that passes its test
    When only the feature file is edited to change the expectation
    Then the next test run recompiles the scenario binary
    And the test fails against the new expectation