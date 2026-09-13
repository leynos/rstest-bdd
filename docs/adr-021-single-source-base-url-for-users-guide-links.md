# Architectural decision record (ADR) 021: record the users-guide base URL once

## Status

Accepted (2026-09-13): Record the canonical base URL once, in
`scripts/users_guide_links.py`, and generate the reference block in
`docs/users-guide.md` from it. `make update-users-guide-links` writes the
block; `make lint` runs the same script without `--fix` and fails while the
committed block disagrees with what generation would write.

## Date

2026-09-13.

## Context and problem statement

`docs/users-guide.md` is vendored into consumer projects, so it cannot rely on
relative paths for cross-references into this repository. Issue
[#499][issue-499] moved those cross-references to absolute GitHub URLs,
collected as reference-style definitions at the bottom of the guide. Pull
request [#521][pr-521] added `scripts/check_users_guide_links.py` to stop those
absolute URLs rotting silently, and [ADR-014][adr-014] retained it as a
proportionate gate.

The checker detects drift, but it does not remove it. The base URL was written
in two places — the checker's `BASE_URL` constant and every reference
definition in the guide — so renaming the default branch or relocating the
repository meant editing the constant and each definition, with the gate
reporting every site that was missed. Issue [#537][issue-537] asked for
generation alongside the check, so the base URL is recorded once and the
definitions are derived from it.

## Decision drivers

- A default-branch rename or a repository move must cost one constant and one
  command, not one edit per definition.
- The committed guide must stay plain Markdown: it renders on GitHub and in
  consumer projects with no preprocessing step, so no placeholders or template
  syntax may survive into it.
- The gate must stay deterministic and must still fail when a link rots.
  Regenerating the block over a fault, or comparing the block against itself,
  would satisfy the letter of the check while removing its value.
- The base URL must not be inferred from whatever the committed block happens
  to say: it is recorded, and the block is derived from it.

## Options considered

### Option A: `--fix` on the existing checker, driven by a Makefile target

Add a `--fix` mode that rewrites the definition URLs the recorded base
determines, and a `make update-users-guide-links` target that runs it.
`make lint` keeps running the script without `--fix`, so the gate fails while
the committed block disagrees with the generated one.

Pros:

- One place records the base URL, and one command rewrites the block.
- The check stays the same script: `make lint` compares the committed text
  against generated text rather than against a second copy of the rule.
- Mirrors the existing `check_fixture_lockfiles.py --refresh` /
  `update-fixture-lockfiles` pair, so the workflow is already familiar.

Cons:

- The script carries both directions, so it must distinguish a definition it
  can rewrite from one it can only report.

### Option B: render the guide from a template

Keep the definitions in a source document and render `docs/users-guide.md`
from it.

Pros:

- The base URL appears only in the source, and generation is total.

Cons:

- Every contributor edit to the guide becomes a template-plus-render cycle, and
  a generated file invites edits that the next render discards.
- Adds a build step to the guide's lifecycle for the sake of one URL prefix.
- The published artefact is the Markdown, so review would inspect generated
  text while the source drifts.

Rejected: disproportionate to linking one document.

### Option C: keep detection only, document the two-place edit

Leave the checker as it is and record in the developers' guide that a move
requires editing the constant and the reference block together.

Pros:

- No new code, and no write path that can disturb the guide.

Cons:

- The failure mode this issue reports is unchanged: the edit is repeated for
  every definition, and the gate reports the ones that were missed.
- Drift between the constant and the block stays possible between the edit and
  the fix.

Rejected: it documents the problem rather than removing it.

| Topic                      | Option A | Option B | Option C |
| -------------------------- | -------- | -------- | -------- |
| Base URL recorded once     | Yes      | Yes      | No       |
| Guide stays plain Markdown | Yes      | Yes      | Yes      |
| Extra build step           | No       | Yes      | No       |
| Gate still catches rot     | Yes      | Yes      | Yes      |

_Table 1: Comparison of the generation options._

## Decision outcome

Generate the reference block from one recorded base URL, checked by the command
that writes it.

- `scripts/users_guide_links.py` records `REPOSITORY_URL`, `DEFAULT_BRANCH`,
  and `DOCS_DIR`, and derives the canonical base URL and the block's
  definitions from them.
- `scripts/check_users_guide_links.py --fix` rewrites the guide in place and
  then reports whatever is still invalid, so regeneration cannot hide a missing
  document.
- `make update-users-guide-links` is the write side; `make lint` runs the same
  script without `--fix`.

A definition is recognized by shape, not by the base URL it was written
against: a view URL on the canonical host whose final path segment names a file
in the documentation tree is repaired whatever base URL precedes it. That is
what lets a block written before a branch rename, a repository move, or a
documentation relocation be brought forward. A definition that already carries
the canonical base URL is returned as written, so a target that has gone
missing is reported rather than masked by a rewrite, and a definition under the
repository prefix that names no document — an issue link, for example — is
reported rather than passed off as a third party's link.

Generation rewrites only the URL of a reference definition line, preserving
labels, order, spacing, and line endings, so the guide stays plain Markdown and
a rewrite cannot disturb prose.

## Consequences

- Renaming the default branch, moving the repository, or relocating the
  documentation tree is one constant plus `make update-users-guide-links`; the
  gate then passes, or names the definitions that still disagree.
- Contributors who edit the reference block by hand now see `make lint` fail
  until the block matches what generation writes; the violation names the
  command to run.
- A third-party link on the canonical host whose final path segment happens to
  name one of this repository's documents is treated as this repository's and
  may be rewritten. The guide's external links point at other hosts, and the
  repository prefix covers links this repository owns that name no document, so
  the exposure is a view URL on `github.com` whose last segment collides with a
  documentation file name.
- The validator remains retained and narrowly scoped; see [ADR-014][adr-014].
  This decision changes how the block is maintained, not what is validated.

## References

- Issue [#537][issue-537]: generate the users-guide reference block from a
  single base URL (this decision).
- Issue [#499][issue-499]: move vendored-guide cross-references to absolute
  GitHub URLs.
- Issue [#540][issue-540]: evaluate proportionality of the validator.
- Pull request [#521][pr-521]: add `scripts/check_users_guide_links.py`.
- Pull request [#541][pr-541]: add CLI and property tests, and ADR-014.
- [ADR-014][adr-014]: retain the users-guide link validator.

[adr-014]: adr-014-retain-users-guide-link-validator.md
[issue-499]: https://github.com/leynos/rstest-bdd/issues/499
[issue-537]: https://github.com/leynos/rstest-bdd/issues/537
[issue-540]: https://github.com/leynos/rstest-bdd/issues/540
[pr-521]: https://github.com/leynos/rstest-bdd/pull/521
[pr-541]: https://github.com/leynos/rstest-bdd/pull/541
