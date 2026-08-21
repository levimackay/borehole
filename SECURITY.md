# Security Policy

## Threat model

Borehole opens and analyzes source code repositories that may come from
untrusted or unfamiliar sources: that's the whole point of the tool. It is
designed under the assumption that **a repository you open in Borehole may
be actively malicious**, not just messy.

Specifically, Borehole:

- **Never executes anything found in a repository.** No package scripts
  (`npm install` hooks, `pip` setup scripts), no build scripts, no Git
  hooks, no binaries, no shell commands constructed from repository
  content.
- **Reads Git data exclusively through `libgit2`** (via the `git2` crate),
  never by shelling out to a `git` binary, so a crafted branch name,
  commit message, tag, or `.gitconfig` cannot inject a shell command.
- **Validates every path** that crosses a trust boundary (index storage,
  Tauri IPC) against the opened repository's root, rejecting anything that
  resolves outside it: the defense against path traversal via `..`
  segments or malicious symlinks.
- **Never uploads repository content anywhere** unless you explicitly
  enable the optional AI explanation feature and configure a provider.
  See `PRIVACY.md` for exactly what that would send.

See `docs/ARCHITECTURE.md` → "Security posture" for the implementation
details, and `PRIVACY.md` for the data-handling model.

## Supported versions

Borehole is pre-1.0. Security fixes land on `main` and the latest tagged
release only; there is no long-term-support branch at this stage.

## Reporting a vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Report privately using [GitHub's private vulnerability reporting](https://github.com/levimackay/borehole/security/advisories/new)
for this repository (Security tab → "Report a vulnerability"). This opens a
private advisory visible only to the maintainer until a fix is ready.

If you're unable to use GitHub's private reporting for some reason, email
the maintainer via the contact address on the maintainer's GitHub profile
(github.com/levimackay) with a subject line starting `[borehole security]`.

Please include:

- A description of the vulnerability and its potential impact.
- Steps to reproduce (a minimal repository or code snippet that
  demonstrates the issue is ideal, given Borehole's threat model centers
  on malicious repository content).
- Any suggested fix or mitigation, if you have one.

## What to expect

This is a solo-maintained open-source project, not a company with an SLA.
In good faith, the maintainer aims to:

- Acknowledge a report within 7 days.
- Provide an initial assessment (confirmed / not applicable / needs more
  info) within 14 days.
- Credit reporters in the fix's release notes, unless you prefer to remain
  anonymous.

There is no bug bounty program.
