Feature: typed data table error handling
  Scenario: reporting parsing errors for typed tables
    Given the following invalid users exist:
      | name  | email              | active |
      | Alice | alice@example.com | maybe  |
