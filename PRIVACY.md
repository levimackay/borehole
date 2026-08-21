# Privacy

Borehole is local-first by design. This document says exactly what it does
and does not do with your data.

## No account, no cloud, no telemetry

- There is no sign-up, no account, and no login.
- There is no Borehole backend server. The desktop app and CLI talk to
  your filesystem and your local Git repository, nothing else, by
  default.
- Borehole sends no telemetry, analytics, or crash reports anywhere. There
  is no tracking of what repositories you open, what you search for, or
  how you use the app.

## What Borehole reads

- The files inside a repository you explicitly open (via "Open Repository"
  in the desktop app, or the path you pass to the CLI). Borehole never
  scans directories you haven't opened, and never walks above the
  repository root you point it at.
- That repository's local `.git` directory, to mine commit history,
  authorship, and file evolution. This is read-only: Borehole never
  writes to `.git`, never creates commits, and never fetches from or
  pushes to any remote.

## What Borehole writes

- An index database at `<repository>/.borehole/index.db` (SQLite):
  derived data (parsed symbols, resolved references, cached graph edges)
  rebuildable at any time from the repository itself. This directory is
  gitignored by default in Borehole-managed projects; it never needs to be
  committed or shared.
- Optional user settings in your OS's standard application config
  directory (not inside any repository).

## Network access

By default, Borehole makes **zero** network requests. The only two
features that would ever make one are both off unless you turn them on:

1. **Optional AI explanations.** If you configure an AI provider (a cloud
   API key, or a local endpoint like Ollama) in Settings, selecting
   "Explain with AI" sends the evidence Borehole has already gathered
   about the selected symbol (its name, signature, caller/callee counts,
   related commit summaries, never raw secrets, never your full source
   tree) to that provider to generate a natural-language explanation. The
   exact payload is visible in Settings before you enable the feature. AI
   output is always visually labeled "AI-generated" and never presented as
   repository evidence.
2. **Optional update check.** Not yet implemented; when it ships, it will
   be an explicit, off-by-default setting that checks GitHub Releases for
   a newer version. No usage data is attached to that request.

## Secrets

Borehole may detect that code *references* a configuration value or
environment variable (e.g. "uses environment variable `DATABASE_URL`").
It only ever displays the **name** of such a reference, never its resolved
value. Borehole does not read your `.env` files' contents into any report,
export, or AI payload.

## Questions

Open a [GitHub issue](https://github.com/levimackay/borehole/issues) or see
`SUPPORT.md`.
