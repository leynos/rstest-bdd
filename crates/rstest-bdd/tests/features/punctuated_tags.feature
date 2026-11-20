Feature: Tags containing colons and full stops

  @stage:final
  Scenario: Final scenario
    When the scenario labelled "final" runs

  @stage:draft
  Scenario: Draft scenario
    When the scenario labelled "draft" runs

  Scenario Outline: Versioned examples
    When the scenario labelled "<label>" runs

    @v1.1
    Examples: earlier release
      | label |
      | v1.1  |

    @v1.2
    Examples: later release
      | label |
      | v1.2  |
