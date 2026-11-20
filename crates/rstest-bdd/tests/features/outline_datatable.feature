Feature: Data tables inside scenario outlines

  Background:
    Given the background table:
      | key   | value |
      | shelf | north |

  Scenario Outline: Step tables receive substituted Examples values
    When the outline table is:
      | item   | count   |
      | <item> | <count> |

    Examples:
      | item   | count |
      | apples | 3     |
      | pears  | 7     |
