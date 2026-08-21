# Third-Party Notices

Borehole is built on the open-source projects below. This list covers
direct dependencies as of the current release; transitive dependencies are
pulled in automatically by Cargo and npm and are not individually listed
here. Run `cargo metadata` / `npm ls` for the full transitive tree, or
`cargo install cargo-license && cargo license` for a complete generated
report before cutting a release.

All licenses listed here were verified against the crates.io / npm registry
metadata at the time this file was written (2026-08-20), not assumed.

## Rust (Cargo workspace)

| Crate | License |
|---|---|
| serde / serde_json | MIT OR Apache-2.0 |
| anyhow | MIT OR Apache-2.0 |
| thiserror | MIT OR Apache-2.0 |
| rayon | MIT OR Apache-2.0 |
| ignore | Unlicense OR MIT |
| tracing / tracing-subscriber | MIT |
| tree-sitter | MIT |
| tree-sitter-rust | MIT |
| tree-sitter-typescript | MIT |
| tree-sitter-javascript | MIT |
| tree-sitter-python | MIT |
| tree-sitter-go | MIT |
| git2 (libgit2 bindings) | MIT OR Apache-2.0 |
| rusqlite (bundles SQLite, Public Domain) | MIT |
| clap | MIT OR Apache-2.0 |
| tempfile | MIT OR Apache-2.0 |
| toml | MIT OR Apache-2.0 |
| tauri | Apache-2.0 OR MIT |
| tauri-plugin-opener | Apache-2.0 OR MIT |

## JavaScript/TypeScript (npm)

| Package | License |
|---|---|
| react / react-dom | MIT |
| @tauri-apps/api | Apache-2.0 OR MIT |
| @tauri-apps/plugin-opener | MIT OR Apache-2.0 |
| typescript | Apache-2.0 |
| vite | MIT |
| @vitejs/plugin-react | MIT |

## Bundled data

- **SQLite**: the `rusqlite` crate's `bundled` feature compiles SQLite
  directly into the binary. SQLite is dedicated to the public domain.

Every dependency above is permissively licensed (MIT, Apache-2.0, Unlicense,
or public domain) and compatible with Borehole's own MIT license. This file
is maintained by hand and reviewed before each release; it is not a
substitute for running an automated license-compliance scan against the
full dependency tree.
