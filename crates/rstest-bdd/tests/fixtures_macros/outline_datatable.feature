Feature: Data tables in outlines

  Background:
    Given a background table:
      | key |
      | bg  |

  Scenario Outline: Step tables in an outline
    When an outline table:
      | item   |
      | <item> |

    Examples:
      | item   |
      | apples |
