Feature: GPUI stateful window harness

  Scenario: Reconstruct visual context from durable handles
    Given a fresh GPUI window is opened
    When the view is updated through a reconstructed visual context
    Then the durable handles still identify the updated view

  Scenario: Opening a second GPUI window starts from reset state
    Given a fresh GPUI window is opened
    Then no stale handles from a previous scenario remain
