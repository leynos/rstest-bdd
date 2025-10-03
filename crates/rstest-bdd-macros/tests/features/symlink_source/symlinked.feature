Feature: Symlink coverage
  Scenario: Macro sees symlinked directory
    Given a symlinked directory exists
    When the scenarios macro walks the tree
    Then it discovers the symlinked feature
