Feature: String type hint
  Scenario: Parse quoted string with double quotes
    Given the message is "Hello, World!"
    Then the parsed message is "Hello, World!"

  Scenario: Parse quoted string with single quotes
    Given the message is 'Single quoted'
    Then the parsed message is "Single quoted"

  Scenario: Parse empty quoted string
    Given the message is ""
    Then the parsed message is ""
