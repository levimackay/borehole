# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/) once
it reaches 1.0.0. Before 1.0.0, minor version bumps may include breaking
changes.

## [Unreleased]

### Added

- Initial project scaffold: Cargo workspace (`engine-core`, `engine-parse`,
  `engine-index`, `engine-graph`, `engine-git`, `engine-evidence`,
  `cli`), Tauri 2 + React/TypeScript desktop app shell.
- `engine-core`: repository access, path safety, `.gitignore`-aware
  walking, language detection, configuration loading.
- SQLite index schema for symbols, references, and imports.
- CLI skeleton (`borehole analyze|symbol|callers|callees|impact|history|tests|explain`).
- Project documentation: architecture, security, privacy, contributing,
  name/trademark screening.

This is a pre-release, active-development project. Nothing has shipped as
a tagged version yet — see the repository's commit history for granular
progress until the first `v0.1.0` tag.
