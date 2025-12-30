Feature: Python streaming parser

  Scenario: Events decode into published structs
    Given the tei_rapporteur Python module is initialised for streaming
    And the paragraph TEI fixture
    When I stream parse the events
    Then all events decode into msgspec Event instances
