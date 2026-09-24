# Architectural decision record (ADR) 022: a separate compiler-cache family for the Windows publish dry run

## Status

Accepted (2026-09-23): Give the Windows publish dry run's compiler objects an
archive family of their own, `sccache-publish-v1`. `ci.yml`'s Windows
default-features lane writes it on a push to `main`, and both Windows
pull-request lanes restore it into the same `.sccache` directory as the
instrumented family. The two families share that one directory, which is the
single recorded exception to one owner per cached path. The exception holds
because each family's writer restores only its own family.

## Date

2026-09-23.

## Context and problem statement

Every cache in the merge gate is an archive with one owner per path and one
writer per key, as the developers' guide's "Cache ownership" section records.
The Windows lanes have no sccache backend. They use the local `.sccache`
directory, which the cache action restores and saves.

Pull request [#796][pr-796] moved that archive's only writer from `ci.yml`'s
trunk push lane to `coverage-main.yml:coverage-baseline-windows`, at a `v2` key.
`ci.yml`'s trunk run no longer built the instrumented workspace, so an archive
written there would have left every pull-request coverage build cold. The move
had a cost, though. The archive now holds only instrumented objects, and the
publish dry run's package-verification builds are not instrumented, so they
hash differently and reuse none of it. Issue [#803][issue-803] measured the
pull-request Windows `Publish dry run` step at a 678-second mean over six legs
before the move and 862 seconds over four legs after it. That is about 184
seconds more per leg, or six minutes per pull request.

The question is how to persist the dry run's objects without giving a key two
writers and without breaking the one-owner rule silently.

## Decision drivers

- One writer per key. Two writers race for the reservation, and the loser's
  objects are lost.
- Each archive should hold only the objects of the build that writes it. An
  archive that carries a copy of the other family doubles what every pull
  request downloads.
- The writer must be the job that runs the build. On the trunk, only `ci.yml`
  runs `make publish-check`, and only `coverage-main.yml` runs the instrumented
  build.
- The rule must stay checkable. The workflow contracts read steps and
  evaluate guards, so the design has to be expressible as steps and conjunctive
  guards.
- The end-of-job sccache report should keep describing the whole job.

## Options considered

### Option A: two key families overlaid on one directory

Add the `sccache-publish-v1-<scope>-<toolchain>-<lockfile hash>` family.
`ci.yml`'s Windows default-features lane saves it on a push to `main`, after
`Publish dry run`, when its restore missed. Both Windows pull-request lanes
restore both families into `.sccache` before sccache first compiles, so one
server reads both.

The instrumented restore runs on Windows for pull requests only, because only
their coverage steps build that shape in `ci.yml`. The writer's trunk run
therefore restores the publish family alone, and its archive holds only the dry
run's objects.

The cost is a recorded exception to one owner per path, together with contracts
that keep it narrow.

### Option B: a second directory for the dry run

Point `SCCACHE_DIR` at a second directory for the dry-run step. Each family
then owns its own path, and no exception is needed. But a running sccache
server keeps the directory it started with, so the job would have to stop the
server before the dry run and start another. The final statistics report would
then cover only the dry run, and the step's environment would need an
expression that differs between the Linux backend and the Windows directory.

### Option C: one writer that builds both shapes

Have `coverage-baseline-windows` also run `make publish-check` on the trunk, so
one archive holds both families under the existing key. That adds a full dry
run to every trunk push, puts a second copy of the publish configuration in
`coverage-main.yml`, and makes the trunk publisher a packaging job.

### Option D: accept the regression

Keep a single archive and absorb about six minutes per pull request.

| Topic                         | Option A            | Option B            | Option C           | Option D  |
| ----------------------------- | ------------------- | ------------------- | ------------------ | --------- |
| Warm dry run on pull requests | yes                 | yes                 | yes                | no        |
| Writers per key               | one                 | one                 | one                | one       |
| One owner per path            | recorded exception  | holds               | holds              | holds     |
| Extra trunk work              | none                | none                | a second dry run   | none      |
| sccache server handling       | unchanged           | restart mid-job     | unchanged          | unchanged |
| Download per pull-request leg | both families, once | both families, once | one larger archive | one       |

_Table 1: Trade-offs between the four approaches._

## Decision outcome

Option A. It warms the dry run with no extra trunk work and no change to how
the sccache server runs. It also keeps each archive to the objects of the job
that writes it. The price is one exception to the one-owner rule, and the
contracts make that exception exact:

- `runner_cache_test.py` names the one owner pair and the one path they may
  share. Any other collision still fails.
- `coverage_publisher_setup_test.py` allows one writer per platform for each
  family.
- `publish_dry_run_cache_test.py` evaluates the guards through
  `guard_conditions.admits` for every leg and event. The writer saves only on
  the Windows default-features leg's trunk miss, and only after the dry run. In
  every run that writes a family, the restores into `.sccache` restore that
  family and nothing else. Both Windows pull-request legs restore both
  families, and no Linux leg restores the publish family. The test also runs
  both key scripts to show that neither `restore-keys` prefix matches the other
  family's key.

## Consequences

- To restore the instrumented family on a pull request without restoring it on
  the writer's trunk run, that restore is split into a Windows step that runs
  for pull requests only and a Linux step for the local-directory fallback. The
  step ids become `sccache-windows` and `sccache-linux`, in `coverage-main.yml`
  too, because the drift contract holds that workflow's repeated steps equal to
  the gate's.
- On a pull-request lane, the two families share the 4 GB `SCCACHE_CACHE_SIZE`
  budget. The instrumented archive measured 1.17 GB on 2026-09-23. If the union
  ever exceeds the budget, sccache evicts the oldest objects on its first
  write, and raising the size or dividing it between the families becomes the
  next decision.
- The strict Windows leg also restores the publish family on a push, because
  its dry run builds the same packages, but it never writes it.

## References

- Pull request [#796][pr-796]: move CodeScene publication and the Windows
  compiler-cache writer to `coverage-main.yml`.
- Issue [#803][issue-803]: restore a warm trunk archive for the Windows publish
  dry run, with the measurements.
- The developers' guide, sections "Cache ownership" and "Compiler cache".

[issue-803]: https://github.com/leynos/rstest-bdd/issues/803
[pr-796]: https://github.com/leynos/rstest-bdd/pull/796
