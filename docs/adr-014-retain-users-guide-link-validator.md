# Architectural decision record (ADR) 014: retain the users-guide link validator

## Status

Accepted (2026-07-28): Retain `scripts/check_users_guide_links.py`, keep it in
the `make lint` gate, and retain its unit, property, and CLI tests. Limit its
scope to absolute repository-reference definitions in `docs/users-guide.md`.

## Date

2026-07-28.

## Context and problem statement

`docs/users-guide.md` is vendored into consumer projects, so it cannot rely on
relative paths for cross-references into this repository. Issue
[#499][issue-499] moved those cross-references to absolute GitHub URLs,
collected as reference-style definitions at the bottom of the guide. Absolute
URLs do not fail a local build when a target document is renamed, relocated, or
has a heading reworded: the link simply rots, and the rot ships to downstream
consumers who vendored the guide.

Pull request [#521][pr-521] added `scripts/check_users_guide_links.py` to close
that gap, wired into `make lint`. Issue [#540][issue-540] then asked whether a
bespoke validator is proportionate to the risk, or whether the maintenance
burden — a `BASE_URL` constant, GitHub slug derivation, and a test suite —
outweighs the benefit for a single document.

This ADR records the proportionality decision.

## Decision drivers

- Broken links in the vendored guide reach downstream consumers, where they are
  expensive to notice and awkward to attribute back to this repository.
- Drift detection must be deterministic and mechanical; review attention is a
  scarce and unreliable substitute.
- Validation cost must stay proportionate: one document, one canonical base
  URL, and a small script with bounded behaviour.
- The gate must not become a maintenance liability that contributors route
  around.

## Decision outcome

Retain the validator.

- Keep `scripts/check_users_guide_links.py` in the repository.
- Keep it in the `make lint` gate, running against its default root.
- Keep its test coverage: unit and Hypothesis property tests in
  `scripts/tests/test_check_users_guide_links.py`, and Cuprum subprocess/CLI
  integration tests in `scripts/tests/test_check_users_guide_links_cli.py`.

### Scope boundary

The validator checks **only** absolute repository-reference definitions in
`docs/users-guide.md`: that each starts with the canonical `BASE_URL`, resolves
to an existing document under `docs/`, and — where a `#` fragment is present —
matches a heading anchor in the target document. It also fails when the guide
contains no repository references at all, so a reformat cannot silently defang
it.

It does **not** validate documentation cross-references generally. Links in
other documents, relative links, and non-repository URLs (for example docs.rs)
remain outside its remit.

## Rationale

The vendored guide is the one document in this repository whose links are
consumed outside it, so it is the one document where link rot escapes the
normal feedback loop. That asymmetry — not link hygiene in general — justifies
a dedicated check. Restricting the validator to that document keeps the cost
proportionate to the risk it addresses, and keeps the failure mode legible:
when the gate fails, it names the offending definition and why.

Manual review was considered sufficient in principle and insufficient in
practice: it detects nothing deterministically, and it degrades precisely when
review attention is scarcest, during large documentation reshuffles.

## Options considered

### Option A: remove or reduce the validator, accept manual detection

Delete the script (or downgrade it to a warning) and rely on review plus
downstream reports.

Pros:

- Removes a bespoke script, its tests, and the `BASE_URL` constant from the
  maintenance surface.
- One fewer gate to keep green.

Cons:

- Reintroduces exactly the failure that [#499][issue-499] and [#521][pr-521]
  addressed: silent rot shipped to consumers who vendored the guide.
- Detection moves downstream, where it is slowest and most expensive.
- The maintenance saved is small — the script's contract is narrow and stable.

Rejected: the saving is marginal, and it is paid for with the only
deterministic detection mechanism available.

### Option B: expand the validator to all documentation links

Generalize the checker to every cross-reference in `docs/`.

Pros:

- Catches rot everywhere, not just in the vendored guide.

Cons:

- Relative links between documents in the same tree already fail visibly during
  local review and rendering; the marginal benefit is small.
- Broadening the surface means handling many more link shapes, and raises the
  false-positive rate on intentionally external or generated targets.
- Expands a narrow, well-understood gate into an open-ended one without a
  concrete need driving it.

Rejected for now: no concrete broader-link need has arisen. Reassess if one
does.

### Option C: retain, limited to users-guide repository references (selected)

Keep the validator exactly as scoped, with its current gate wiring and test
coverage.

Pros:

- Deterministic drift detection for the one document where rot escapes.
- Bounded maintenance: a single constant, one document, one link shape.
- Failures are specific and actionable.

Cons:

- A bespoke script to maintain, however small.
- Contributors relocating a document must update the guide's reference block.

## Consequences

- `BASE_URL` must be maintained. If the repository moves, the default branch is
  renamed, or documents relocate, update that constant and the guide's
  reference block together.
- The checker continues to validate both target documents and heading
  fragments, so heading rewordings in linked documents surface as gate
  failures. Prefer heading fragments over `#L<n>` line anchors, which break
  silently on reflows.
- CLI and property coverage are retained; see the developers' guide for the
  test split and the Hypothesis/Cuprum tooling rule.
- Expansion to broader documentation links is deferred, and should be
  reassessed only if a concrete need arises rather than pre-emptively.

## References

- Issue [#540][issue-540]: evaluate proportionality of the validator (this
  decision).
- Issue [#499][issue-499]: move vendored-guide cross-references to absolute
  GitHub URLs.
- Pull request [#521][pr-521]: add `scripts/check_users_guide_links.py`.
- Pull request [#541][pr-541]: add CLI and property tests, and record this
  decision.

[issue-499]: https://github.com/leynos/rstest-bdd/issues/499
[issue-540]: https://github.com/leynos/rstest-bdd/issues/540
[pr-521]: https://github.com/leynos/rstest-bdd/pull/521
[pr-541]: https://github.com/leynos/rstest-bdd/pull/541
