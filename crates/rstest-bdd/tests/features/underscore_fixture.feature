Feature: Underscore fixture support

  Scenario: Implicit fixture names normalize one leading underscore
    Given the streaming world is available
    When the parser runs once more
    Then implicit underscore fixture lookup uses the world fixture
    When the underscore-prefixed streaming world is changed
    Then the mutable underscore fixture change is visible
    And explicit from keeps the underscore-prefixed fixture key
