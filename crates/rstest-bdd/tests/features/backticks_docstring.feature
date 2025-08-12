Feature: Doc string with backticks
  Scenario: uses backtick docstring
    Given the following message:
      ```
      hello
      world
      ```
    Then the captured message equals:
      ```
      hello
      world
      ```

