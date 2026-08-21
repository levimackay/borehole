# Borehole website — design spec

## Revision note (v2)

The first build (v1) had a generic centered-hero-plus-two-buttons pattern
(headline, one-line subhead, two side-by-side buttons) that is a named AI
tell. Redesigned via the Hallmark skill's `redesign` flow: three genuinely
different structural directions were rendered (Workbench, Long Document,
Split Studio) and Split Studio was picked. Everything below reflects v2;
the conceit and token/type rules from v1 carry forward unchanged.

## The conceit

**A drill log, not a landing page.** The site reads like the output of the tool itself: evidence-first, monospace-inflected, dense where it has real content, silent where it doesn't. No hero illustration, no abstract gradient — the "art" is the product's own real terminal output and interface, presented like specimens pulled up from a core sample.

## Macrostructure: Split Studio

Diptych. Every major claim divides the screen: statement on one side, real
`borehole` terminal output (or a real screenshot) on the other, pairing
alternates direction down the page. This was picked over Workbench (safe
but conventional dev-tool tour) and Long Document (the most memorable
prose, but it buries the CTA, which is a real risk for a page that needs
to show "how do I download" within seconds) because the *form* of the
page argues the same thing the *product* does: nothing is asserted without
its evidence sitting directly next to it.

Diptych rows are not forced onto every section. Per the reference notes
in `~/.claude/design-references/README.md` ("density alternates hard"),
rows of paired claim+proof alternate with full-width dense sections (the
CLI table, the privacy strip, the architecture diagram) — that alternation
between two-column and full-bleed density *is* the page's rhythm, matching
how the Shopify Editions references swing between a single full-bleed
image/word and a dense two-column reference table.

## Visual identity (inherits the desktop app's tokens, doesn't invent a new one)

- **Palette**: reuse `src/styles/tokens.css` exactly — dark canvas `#121417`, panel `#181b1f`, teal accent `#4fb3ac`, text `#e4e6ea`/`#9aa1ac`/`#7d848f`. The site and the product must look like the same object. Light mode via the same `prefers-color-scheme`/`[data-theme]` mechanism, not a separate palette.
- **Type**: JetBrains Mono for anything code/data/CLI (headings included — the brand voice is technical, not editorial), IBM Plex Sans for body prose. Same two families as the app, self-hosted via `@fontsource`, no new typefaces introduced.
- **No gradients, no bento grids, no dot grids, no glassmorphism, no rounded-pill buttons, no Lucide/sparkle icons, no floating orbs.** Radius stays small (2-3px, matching the app's `--bh-radius-sm/md`) and only on interactive controls.
- **Density over decoration**: real terminal transcripts (monospace, syntax-plain, exactly what `borehole` actually prints — captured, never invented) and a real screenshot of the desktop app are the imagery. No stock photography, no illustrated hero, no fabricated "customer" screenshots.

## Structure (single-page, per the brief)

1. **Hero — diptych.** Left: kicker ("local-first codebase intelligence"), headline "Understand the code before you change it.", one line of supporting text, Download (primary, teal) + View on GitHub (secondary, outline). Right: the real `borehole impact top --before` terminal transcript from the README. Nav is content-sized, not a floating pill (matches the app's own `bh` mark + wordmark).
2. **Evidence model — diptych, reversed** (proof left, claim right this time, so consecutive rows don't all point the same way). The `borehole symbol Handler` ambiguous-refs transcript paired with the "two unrelated classes, one name" argument — the single most differentiating proof point, given a full row to itself.
3. **The problem** — short prose, full-width, not paired with proof (it's scene-setting, not a claim). Matches README's "The problem" verbatim.
4. **Product demo** — full-bleed: the desktop app's command-palette screenshot inside a bordered frame matching the app's own panel styling. The frame IS the app, not a browser-chrome mockup.
5. **Before you change this — diptych.** The reading-list feature gets its own paired row (claim + the real reading-list terminal output), since it's the product's core workflow.
6. **Everything else — full-width dense reference table.** Dependency graph, blast radius, git archaeology, test discovery, context export: a two-column definition-list, not cards. The density break after two diptych rows and a full-bleed image is deliberate.
7. **CLI section** — the command table from the README, full-width, an actual monospace table.
8. **Privacy/Security strip** — three short, factual statements, full-width, dense, no icons.
9. **Architecture** — the mermaid pipeline as a static inline SVG.
10. **Supported languages** — plain list, not badges/pills.
11. **Download** — platform links to GitHub Releases, honest about no tagged release existing yet.
12. **Footer** — GitHub, docs, license, the AI-assisted-development disclosure line verbatim from the README.

## Content/asset inventory — what's real right now

- Real: README's two terminal transcripts, the command-palette screenshot, the search-view screenshot, the mermaid architecture diagram, the CLI command table, all feature copy (already written for the README — reuse verbatim, don't rewrite marketing copy that duplicates it).
- **Not yet real**: no tagged GitHub Release exists yet, so platform download buttons must link to the GitHub Releases page (`https://github.com/levimackay/borehole/releases`) rather than a direct binary URL — do not fabricate a release or a version number. Label the Download section honestly if no release exists yet ("Releases are published on GitHub" is enough — don't pretend a v1.0 exists).
- No fabricated testimonials, customer logos, star counts, or usage numbers anywhere on the page.

## Motion

One staggered reveal on the hero's headline/subhead/CTA on load. Below the
fold: each diptych row's two halves cross-fade in slightly staggered
(claim then proof, ~80ms apart) as the row enters the viewport, once,
never re-triggering on scroll-back — this is the Split Studio
macrostructure's own reveal spec, and it reinforces the pairing rather
than decorating the page. Full-width sections (problem, CLI table,
privacy strip, architecture, languages) do not reveal — they're just
there, per the Long Document principle of density needing no animation
to justify itself. `transform`/`opacity` only, `--ease-out` easing,
`prefers-reduced-motion: reduce` collapses every reveal to an instant
state (no animation, not just a shorter one). No scroll-triggered
parallax, no floating elements, no re-triggering on scroll-up/down.

## Stack

Static site (no backend, per the brief). Plain Vite + vanilla TS/HTML or a minimal React build — whichever is faster to ship correctly; this doesn't need a framework's worth of interactivity. Deploy target: Cloudflare Pages at `borehole.levimackay.com` (DNS + Pages project provisioned directly via the existing scoped Cloudflare API token — no dashboard hand-off needed).
