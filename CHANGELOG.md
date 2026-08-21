# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/) once
it reaches 1.0.0. Before 1.0.0, minor version bumps may include breaking
changes.

## [Unreleased]

### Added

- Full analysis engine: repository indexing (`engine-core`), tree-sitter
  symbol/reference extraction for Rust, TypeScript, TSX, JavaScript,
  Python, and Go (`engine-parse`), a SQLite-backed index with
  import-scoped reference resolution (`engine-index`), bounded
  graph traversal for callers/callees/subclasses (`engine-graph`),
  Git history mining via libgit2: file history, rename tracking,
  temporal coupling (`engine-git`), and evidence synthesis: symbol
  profiles, blast radius with risk scoring, "before you change this"
  reading lists, code evolution (`engine-evidence`).
- CLI (`borehole analyze|symbol|callers|callees|impact|history|tests|explain`),
  every subcommand supporting `--json`.
- Desktop application: Tauri 2 + React/TypeScript shell with a command
  palette, seven views (Explorer, Symbols, Graph, History, Impact,
  Tests, Search), and an IPC layer over the same evidence engine the
  CLI uses.
- 98 tests across the Rust workspace, including integration tests
  against 11 generated fixture repositories covering renames, deleted
  symbols, monorepos, generated files, and, most importantly, a fixture
  proving the reference resolver refuses to guess between two
  same-named, unrelated symbols rather than silently merging them.
- Marketing site at borehole.levimackay.com.
- Project documentation: architecture, security, privacy, contributing,
  name/trademark screening, release-signing status.

### Fixed

- A stack-overflow denial-of-service: adversarially deep source nesting
  could crash the indexing process via unbounded recursive tree
  traversal in `engine-parse`. Converted to explicit-stack iteration
  and added a hard parse-time ceiling (tree-sitter's own parse could
  still take 20+ seconds on pathological input even after that fix).
- A Windows-only linker failure (`libgit2-sys` needing `advapi32.lib`
  APIs that weren't being linked) caught by CI's Windows matrix job.

This is a pre-release, active-development project. Nothing has shipped as
a tagged version yet. See the repository's commit history for granular
progress until the first `v0.1.0` tag.
