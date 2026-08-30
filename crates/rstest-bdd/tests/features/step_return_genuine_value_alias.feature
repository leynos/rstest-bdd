Feature: Genuine value alias step return

  Scenario: A value alias overrides its fixture
    Given the score is 1
    When the score is increased
    Then the score is 2
