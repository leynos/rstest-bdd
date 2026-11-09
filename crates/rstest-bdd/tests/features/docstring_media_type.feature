Feature: Docstring media types
  A media type after the opening delimiter is not part of the docstring.

  Scenario: Annotated docstrings match bare ones
    Given the bare docstring is journalled
      """
      {
        "id": 7
      }
      """
    And the annotated docstring is journalled
      """json
      {
        "id": 7
      }
      """
    And the backtick docstring is journalled
      ```application/json
      {
        "id": 7
      }
      ```
