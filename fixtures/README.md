# Fixtures

Small, deterministic repositories used by integration tests across
`engine-index`, `engine-git`, and `engine-evidence`. Each fixture is a real
Git repository with real commits (not hand-crafted `.git` internals), built
by `fixtures/build-fixtures.sh` so history is reproducible and readable in
`git log`.

**Run `fixtures/build-fixtures.sh` before running tests that depend on
these.** The materialized fixture directories are gitignored, not
committed — a git repository nested inside another becomes a broken
"embedded repository" gitlink rather than tracked files, so the generator
script is the source of truth and CI runs it as a step before `cargo
test`.

A good fixture:

- Tests one specific behavior, named in its directory (e.g.
  `ambiguous-refs` exists solely to prove the resolver refuses to guess
  between two same-named symbols).
- Has a handful of files, not a realistic-sized project — big enough to be
  representative, small enough that a reviewer can read every file in a
  minute.
- Has a `NOTES.md` inside it explaining what it's for and what the
  expected analysis output is, so a future contributor can tell whether a
  test failure means "the fixture broke" or "the engine broke."

Planned fixtures (see `docs/ARCHITECTURE.md` and the project brief for the
full rationale): `simple-ts`, `nested-deps`, `cycle`, `inheritance`,
`rename-history`, `deleted-symbol`, `monorepo`, `generated-files`,
`ambiguous-refs`, `go-basic`, `rust-basic`.
