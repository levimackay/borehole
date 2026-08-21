# Borehole website — design spec

## The conceit

**A drill log, not a landing page.** The site reads like the output of the tool itself: evidence-first, monospace-inflected, dense where it has real content, silent where it doesn't. No hero illustration, no abstract gradient — the "art" is the product's own real terminal output and interface, presented like specimens pulled up from a core sample.

## Visual identity (inherits the desktop app's tokens, doesn't invent a new one)

- **Palette**: reuse `src/styles/tokens.css` exactly — dark canvas `#121417`, panel `#181b1f`, teal accent `#4fb3ac`, text `#e4e6ea`/`#9aa1ac`/`#7d848f`. The site and the product must look like the same object. Light mode via the same `prefers-color-scheme`/`[data-theme]` mechanism, not a separate palette.
- **Type**: JetBrains Mono for anything code/data/CLI (headings included — the brand voice is technical, not editorial), IBM Plex Sans for body prose. Same two families as the app, self-hosted via `@fontsource`, no new typefaces introduced.
- **No gradients, no bento grids, no dot grids, no glassmorphism, no rounded-pill buttons, no Lucide/sparkle icons, no floating orbs.** Radius stays small (2-3px, matching the app's `--bh-radius-sm/md`) and only on interactive controls.
- **Density over decoration**: real terminal transcripts (monospace, syntax-plain, exactly what `borehole` actually prints — captured, never invented) and a real screenshot of the desktop app are the imagery. No stock photography, no illustrated hero, no fabricated "customer" screenshots.

## Structure (single-page, per the brief)

1. **Hero.** Headline: "Understand the code before you change it." One line of supporting text. Two CTAs: Download (primary, teal), View on GitHub (secondary, outline). Below the fold line: a real terminal transcript rendered as a code block (the `borehole impact top --before` session from the README), not a decorative screenshot.
2. **The problem**, stated in 2-3 sentences — matches README's "The problem" section, don't reinvent new marketing copy.
3. **Product demo**: the desktop app screenshot (command palette), full-bleed within a bordered frame matching the app's own panel styling — the frame IS the app, not a browser mockup around it.
4. **Feature grid — but not a 3-card bento.** Alternate a wide full-bleed section (one feature, large type, generous space — e.g. "Before You Change This") with a dense two-column reference-table-style section (the rest: dependency graph, blast radius, git archaeology, evidence model, test discovery, context export) so the page doesn't read as uniform equal-weight cards.
5. **Evidence model callout**: the ambiguous-refs terminal transcript from the README (`borehole symbol Handler` showing two unresolved same-named classes) as a standalone, large-type section — this is the single most differentiating proof point the product has, give it room.
6. **CLI section**: the command table from the README, rendered as an actual monospace table, not prose.
7. **Privacy/Security strip**: three short, factual statements (no account · no cloud · git access via git2, never a shell) — dense, not padded with icons.
8. **Architecture**: the mermaid diagram from the README, or a simplified static SVG version of the same pipeline.
9. **Supported languages**: plain list, not badges/pills.
10. **Download**: platform links pointing at GitHub Releases — see Content/asset inventory below for what's real vs. not-yet-real.
11. **Footer**: GitHub, docs, license, "AI-assisted development disclosed" line (same wording as README, since this is the same disclosure, not new copy).

## Content/asset inventory — what's real right now

- Real: README's two terminal transcripts, the command-palette screenshot, the search-view screenshot, the mermaid architecture diagram, the CLI command table, all feature copy (already written for the README — reuse verbatim, don't rewrite marketing copy that duplicates it).
- **Not yet real**: no tagged GitHub Release exists yet, so platform download buttons must link to the GitHub Releases page (`https://github.com/levimackay/borehole/releases`) rather than a direct binary URL — do not fabricate a release or a version number. Label the Download section honestly if no release exists yet ("Releases are published on GitHub" is enough — don't pretend a v1.0 exists).
- No fabricated testimonials, customer logos, star counts, or usage numbers anywhere on the page.

## Motion

One staggered reveal on the hero's headline/subhead/CTA on load. Nothing else animates except `prefers-reduced-motion`-respecting hover/focus state transitions on interactive elements (buttons, links). No scroll-triggered parallax, no floating elements.

## Stack

Static site (no backend, per the brief). Plain Vite + vanilla TS/HTML or a minimal React build — whichever is faster to ship correctly; this doesn't need a framework's worth of interactivity. Deploy target: Cloudflare Pages at `borehole.levimackay.com` (DNS + Pages project provisioned directly via the existing scoped Cloudflare API token — no dashboard hand-off needed).
