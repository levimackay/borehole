# Contributing to Borehole

Thanks for considering a contribution. Borehole is a young project: the
architecture described below is the current shape, and it's expected to
evolve as real usage finds its rough edges.

## Development setup

Prerequisites:

- Rust (stable, see `rust-version` in `Cargo.toml` for the floor) via
  [rustup](https://rustup.rs)
- Node.js 20+ and npm
- Platform build dependencies for Tauri 2: see the
  [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)
  for your OS (on Linux this means WebKitGTK + a few dev packages; macOS
  and Windows need only their standard toolchains)

```sh
git clone https://github.com/levimackay/borehole.git
cd borehole
npm install
cargo build --workspace
```

Run the desktop app in development mode:

```sh
npm run tauri dev
```

Run the CLI directly:

```sh
cargo run -p borehole -- analyze .
```

## Architecture

Read `docs/ARCHITECTURE.md` before making non-trivial changes. It
documents the crate boundaries, the reference-resolution algorithm, the
SQLite schema, and the "adding a language" checklist. The short version:
`engine-core` has no internal dependencies, `engine-parse`/`engine-git`
depend only on it, `engine-index`/`engine-graph` build on `engine-parse`,
and `engine-evidence` synthesizes everything for both `crates/cli` and
`src-tauri` to consume identically. Don't introduce a dependency edge that
violates that direction.

## Testing

```sh
fixtures/build-fixtures.sh   # materialize the fixture repos (generated, not committed)
cargo test --workspace       # Rust
npm run build                # frontend typecheck + build
```

New logic in `engine-*` crates should ship with unit tests. Changes to
cross-file behavior (resolution, indexing, git history) should exercise
the fixtures under `fixtures/` where a relevant one exists, or add a new
one. See `fixtures/README.md` for what makes a good fixture and why
they're generated rather than committed as nested repos.

## Formatting and linting

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

CI enforces both; a PR with clippy warnings won't pass.

## Commit conventions

Write commit messages that explain *why*, not just *what*: the diff
already shows what changed. Keep commits logically scoped (one concern per
commit) rather than one giant commit per PR.

## Pull requests

- Reference the issue you're addressing, if any.
- Describe what changed and why in the PR description; the template will
  prompt for this.
- Keep the diff focused; unrelated cleanup belongs in its own PR.
- Make sure `cargo fmt`, `cargo clippy`, and the test suite are clean
  before requesting review. CI will catch it either way, but it's faster
  for everyone if it's clean going in.

## Adding a language

See `docs/ARCHITECTURE.md` → "Adding a language" for the four-step
checklist (new `Language` variant, grammar dependency, `LanguageExtractor`
implementation, wire it into the dispatch table). Nothing downstream of
`engine-parse` needs to change.

## Reporting bugs and requesting features

Use the GitHub issue templates. For security vulnerabilities, see
`SECURITY.md` instead. Do not open a public issue.

## Code of Conduct

This project follows the Contributor Covenant. See `CODE_OF_CONDUCT.md`.
