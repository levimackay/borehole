# Architecture

Borehole is a Cargo workspace of small, single-direction crates plus a React/TS
frontend. The crate boundaries *are* the pipeline described in the project
brief: repository access → parsing → symbol extraction → indexing →
relationship analysis → git analysis → evidence engine → application API →
UI.

```
engine-core  (types, config, ignore rules, repo access)
     |
     +--> engine-parse   (tree-sitter parsing + symbol/reference extraction)
     |         |
     |         v
     +--> engine-index   (SQLite persistence, reference resolution)
     |         |
     |         v
     |    engine-graph   (traversal: callers/callees/subclasses)
     |
     +--> engine-git     (git2-based history, independent of parse/index)
     |
     +----------------------------------------+
                                               v
                                     engine-evidence
                                     (synthesizes index+graph+git into
                                      cited SymbolProfile / BlastRadius /
                                      BeforeYouChangeThis / CodeEvolution)
                                               |
                                  +------------+------------+
                                  v                         v
                            crates/cli                 src-tauri
                            (`borehole` binary)   (desktop app IPC layer)
                                                              |
                                                              v
                                                          src/ (React/TS)
```

Dependency direction is strictly one-way and enforced by Cargo.toml, not just
convention: `engine-core` depends on nothing else in the workspace.
`engine-git` depends only on `engine-core` (it reads `.git` directly and
never touches the parse/index pipeline). `cli` and `src-tauri` both depend
on `engine-evidence` and nothing lower — that's what guarantees the CLI and
the desktop app produce identical answers to the same question, because
they're calling the same functions, not two reimplementations.

## Confidence is not optional

Most of what this tool reports isn't computed by a compiler or a type
checker — it's resolved by name-and-import-scope heuristics (see
`crates/engine-index/src/resolve.rs`) and by intersecting git diff hunks
with symbol spans (see `crates/engine-git`). That's a deliberate scope
decision (full semantic resolution per language is a language-server-scale
project on its own), not an oversight, and it means every relationship the
engine reports carries an explicit `engine_core::Confidence`
(`High`/`Medium`/`Low`).

The rule for anyone implementing or extending this engine: **a claim
without evidence, or evidence you're not sure of, gets a low confidence
tag and stays in the output — it never gets silently upgraded or
dropped.** The UI is responsible for surfacing confidence honestly ("these
areas *may* be affected", never "this *will* break").

## Reference resolution algorithm

Implemented in `crates/engine-index/src/resolve.rs`. In order:

1. Same-file symbol with a matching name → `High` confidence.
2. Exactly one import binds the name → resolve the import's source module to
   a file, look up the symbol there → `High` if found, `Medium` if the
   module resolved but the symbol didn't (e.g. an unfollowed re-export).
3. Exactly one symbol *anywhere* in the index has this name → `Medium`.
4. More than one symbol anywhere shares the name → **do not guess**; record
   `to_symbol: None` at `Low` confidence. See the `fixtures/ambiguous-refs`
   fixture — silently merging two unrelated same-named methods is a
   correctness bug that would poison blast-radius counts, not a helpful
   fallback.
5. Nothing found → `to_symbol: None`, `Low` confidence. Expected and
   harmless for calls into external dependencies outside the repo.

## SQLite schema

`crates/engine-index/src/schema.rs` is the single source of truth — don't
duplicate table shapes elsewhere. Four tables: `files`, `symbols`, `refs`,
`imports`, plus a `meta` key/value table holding `schema_version`. Foreign
keys cascade on file deletion, so re-indexing a changed file is "delete old
rows for this file_id, re-insert" rather than a diffing dance. The database
lives at `<repo_root>/.borehole/index.db` and is never committed (see
`.gitignore`) — it's derived data, rebuildable from the repo + its git
history at any time.

## Adding a language

1. Add the `Language` variant in `crates/engine-core/src/language.rs`
   (extension mapping + the enum itself).
2. Add the tree-sitter grammar crate to `crates/engine-parse/Cargo.toml`
   (workspace dependency).
3. Implement `LanguageExtractor` in a new
   `crates/engine-parse/src/languages/<lang>.rs`, wire it into
   `languages::extractor_for`.
4. Nothing in `engine-index`, `engine-graph`, `engine-git`, or
   `engine-evidence` needs to change — they're language-agnostic by
   construction (they operate on `ParsedSymbol`/`ParsedReference`, not on
   language-specific AST shapes).

v1 ships Rust, TypeScript/TSX, JavaScript, Python, and Go — a deliberate
subset chosen for tree-sitter grammar maturity and coverage of this
project's own stack, per the brief's explicit permission to not attempt
every language at once (section 10). Java, C#, and Swift are documented
roadmap items with the same four-step path.

## Security posture

- Git access is exclusively via `git2` (libgit2), never by shelling out to
  a `git` binary — a malicious repo cannot inject shell commands through a
  crafted branch name, commit message, or `.gitconfig`.
- Borehole never executes anything found in a repository: no package
  scripts, no build scripts, no git hooks, no binaries. It only *reads*
  file contents and git object data.
- Every path that crosses a trust boundary (a path stored in the SQLite
  index, a path arriving over Tauri IPC) is resolved through
  `RepoHandle::resolve`, which canonicalizes and rejects anything that
  escapes the repository root — the defense against path traversal via
  `..` segments or symlinks planted in a malicious repo.
- Config values shown in the UI (e.g. "uses environment variable
  `DATABASE_URL`") show the *name*, never the resolved value — see
  `EvidenceEngine::config_touches`.
- Full threat model: see `SECURITY.md`.

## Application API / IPC contract

Tauri commands in `src-tauri` and CLI subcommands in `crates/cli` are two
thin, structurally identical wrappers over `engine-evidence`:

| Capability | CLI | Tauri command |
|---|---|---|
| Open a repo | `borehole analyze <path>` | `open_repository` |
| Symbol search | `borehole symbol <name>` | `search_symbols` |
| Callers | `borehole callers <symbol>` | `get_callers` |
| Callees | `borehole callees <symbol>` | `get_callees` |
| Blast radius | `borehole impact <path\|symbol>` | `get_blast_radius` |
| Before you change this | (part of `impact --before`) | `get_before_you_change_this` |
| History | `borehole history <path\|symbol>` | `get_history` |
| Related tests | `borehole tests <symbol>` | `get_related_tests` |
| Explain (evidence summary) | `borehole explain <path\|symbol>` | — (composed client-side from the above) |
| Context export | `--json` on any command | `export_context` |

Every CLI subcommand supports `--json` for machine-readable output (brief
section 27), which is the same serde-serialized shape the Tauri commands
return — one set of types (`engine-evidence`'s report structs), two
transports.

TypeScript types in `src/lib/ipc-types.ts` mirror these Rust `serde`
structs field-for-field; `src/lib/ipc.ts` wraps `invoke()`/`listen()` with
those types so components never call raw `invoke()`.

## Performance posture

- Indexing walks files in parallel (`rayon`) and reports progress via a
  callback (`IndexProgress`) so the CLI can render a bar and the desktop
  app can emit a Tauri event — neither blocks the UI thread.
- Re-indexing is incremental: a file's content hash is compared against
  the stored `files.content_hash`; unchanged files are skipped entirely,
  including their symbol re-extraction.
- Graph traversal (`engine-graph`) is depth-bounded (`BoreholeConfig::max_graph_depth`,
  default 6) and cycle-safe — a symbol is never re-visited as a node even
  if reachable by multiple paths.
- Git history queries accept an optional commit limit
  (`BoreholeConfig::git_history_limit`) to bound worst-case walk time on
  repositories with tens of thousands of commits.

## AI layer (optional, off by default)

Not yet implemented in v1. When added, it will be a trait
(`AiProvider::explain(evidence: &Explainable) -> String`) in a new
`crates/engine-ai` crate with Anthropic and OpenAI-compatible
implementations, invoked only when a user explicitly enables it in
Settings. AI output is always rendered in the UI under a visually distinct
"AI-generated explanation" label, never merged into evidence-labeled text.
See `PRIVACY.md` for what would be sent to a provider once this ships.
