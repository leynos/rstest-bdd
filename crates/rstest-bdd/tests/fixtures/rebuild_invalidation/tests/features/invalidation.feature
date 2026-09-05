# WARNING — how the 10.3.3 regression test mutates this file:
#
# The expensive rebuild-behaviour test rewrites ONLY the plain `100` on the
# `Then` step, changing it to `101`. Two CI legs build with
# `strict-compile-time-validation`, under which changing step pattern or
# keyword text would fail as a *compile* error, so this file must never be
# edited in any way that alters the step keywords ("Given" / "Then") or the
# pattern text ("the captured value is", "the bound expectation is"). Only the
# numeric argument value `100` may change. The step pattern is
# `the bound expectation is {value:u32}`; `100` is the bound argument value,
# not part of the pattern.

Feature: Rebuild-invalidation fixture

  Scenario: a captured expectation
    Given the captured value is 100
    Then the bound expectation is 100