# Provenance of the imported OrthoConfig v0.10.0 guides

This document records where the two imported OrthoConfig guides came from. It
exists so that a reader can confirm the imported text has not drifted from the
upstream release without having to trust the commit that added it.

The imported files are verbatim copies. Their text is not edited locally, not
reformatted, and not corrected. Anything this repository needs to say about
OrthoConfig beyond the upstream text belongs in the design document, in an
architectural decision record (ADR), or in the users' guide, not in the
imported files.

## Source revision

The guides were taken from the `leynos/ortho-config` repository at the
`v0.10.0` release tag.

_Table 1: The resolved source revision._

| Item           | Value                                                                 |
| -------------- | --------------------------------------------------------------------- |
| Repository     | <https://github.com/leynos/ortho-config>                              |
| Tag            | `v0.10.0`                                                             |
| Tag object     | `996e5a1c9f22ace4f55bf5dcc196d2c0524262bf`                            |
| Commit         | `38812c563e3953a490e3efc202609f37e21a0517`                            |
| Commit subject | `Prepare the 0.10.0 release and test every documented example (#620)` |

The tag is annotated: its object identifier differs from the commit it peels
to. Both identifiers were resolved independently of the issue text by fetching
`refs/tags/v0.10.0` into a fresh clone and reading the tag object directly. The
resolved pair matches the pair named in issue #901 exactly, so there is no
differing tag resolution to report.

The import was made from the commit, not from the tag name and not from a
branch or a latest-documentation URL, so re-tagging the repository upstream
cannot silently change what this repository records.

## Imported files

_Table 2: Source paths, destination paths, and checksums._

| Upstream path                     | Repository path                                               | SHA-256                                                            |
| --------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------ |
| `docs/users-guide.md`             | `docs/ortho-config-users-guide.md`                            | `84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575` |
| `docs/v0-10-0-migration-guide.md` | `docs/ortho-config-v0-10-0-migration-guide.md`                | `b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf` |
| `docs/users-guide.md`             | `vendor/ortho-config-v0-10-0/docs/users-guide.md`             | `84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575` |
| `docs/v0-10-0-migration-guide.md` | `vendor/ortho-config-v0-10-0/docs/v0-10-0-migration-guide.md` | `b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf` |

The checksums were taken from the upstream files at commit `38812c56` and
re-taken from both the requested flat imports and the link-preserving copies;
all four checksums match. To check a copy, run `sha256sum` on the repository
path and compare it with Table 2.

The imported guides contain relative links to one another using their upstream
filenames (`users-guide.md` and `v0-10-0-migration-guide.md`). The repository's
project guide already owns `docs/users-guide.md`, so those names cannot be
restored beside the requested flat imports without changing its meaning. Exact
copies with upstream filenames therefore live at
`vendor/ortho-config-v0-10-0/docs/`. The documentation index points to these
copies, whose relative links resolve within the pinned upstream layout.

The requested flat imports under `docs/` retain their upstream-relative links
unchanged. In this repository, those links can resolve to unrelated project
files or to paths that are absent. The flat imports are retained as the exact
source copies and are not linked from `docs/contents.md`; follow links from the
vendor copies, which are the indexed, link-safe versions.

The mirror also contains short pointer pages for relative upstream targets
outside the two imported guides. Each pointer leads to the corresponding file
or directory at the immutable source commit. The imported text in both
locations remains byte-for-byte identical; compare the four files with the
checksums in Table 2.

## Version boundary

Both guides describe OrthoConfig v0.10.0 and nothing later. Statements in the
imported text that a capability is not yet available, or is planned, are
accurate for v0.10.0 and must not be read as describing a later release. The
guides are not updated in step with upstream `main`.

One consequence is recorded here because it constrains the integration and is
easy to assume otherwise: OrthoConfig v0.10.0 does not build with
`default-features = false` (upstream issue leynos/ortho-config#627), so the
dependency is taken with its default features enabled.
