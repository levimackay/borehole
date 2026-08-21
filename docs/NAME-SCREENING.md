# Name Screening (2026-08-20)

## Prior Art / Landscape

- **CodeQL** (GitHub/Microsoft): builds a queryable relational database from source and runs semantic dataflow/taint-tracking queries. Analysis-first, security-focused (SAST). Cross-platform, CLI + CI-oriented, no desktop app. Deep but slow (minutes to 30+ min per scan). Doesn't do temporal/git-history mining, test discovery, or "blast radius before you touch this" synthesis: it answers "does this pattern of vulnerable dataflow exist," not "what will break if I change this function."
- **Sourcegraph**: server/cloud "universal code search" platform: cross-repo semantic search, LSIF/SCIP-based jump-to-def and find-references, batch changes for org-wide migrations. Analysis-first, enterprise/team-deployed (Kubernetes), not local-first or single-developer desktop. No git-history temporal coupling mining or synthesized change-risk reports; it's navigation and search infrastructure, not a "before you change this" advisor.
- **Semgrep**: fast, pattern-based (YAML rule) static scanner, 30+ languages, seconds per scan, lightweight vs. CodeQL's deep semantic model. CI/CLI-first, analysis-first. Finds pattern matches (bugs, security anti-patterns), not dependency/call graphs, git evolution, or blast radius.
- **Understand (SciTools)**: desktop static-analysis IDE/tool covering 70+ languages; builds a "UDB" entity database and renders 50+ graph types (dependency, call, butterfly, UML). Closest prior art structurally (desktop app, symbol/dependency graphs, cross-language), but it's a standalone code-metrics/refactoring IDE with no git-history mining, no test-discovery, no synthesized evidence-backed reports, and it's commercial/closed rather than local-first OSS with a CLI-first architecture.
- **SonarQube**: the long-standing code-quality/security static-analysis platform (bugs, vulnerabilities, code smells, 6,500+ rules), on-prem or cloud, PR-gate oriented. Quality-gate/linting-first, not codebase-comprehension or change-impact-first; no temporal coupling or blast-radius synthesis.
- **CodeSee**: built interactive codebase maps and "code tours" for onboarding/change visualization; acquired by GitKraken in 2024 and effectively wound down as a standalone product. Visualization-first, web-based, not local-first, no git-mining or evidence citations.
- **Swimm**: AI-assisted documentation tool that keeps docs/tutorials in sync with code changes via IDE plugins (VS Code/JetBrains); closest living heir to CodeSee's onboarding niche. Documentation-first, AI-assisted, not analysis/evidence-first, no blast-radius or dependency-graph synthesis.
- **Sourcetrail (discontinued)**: free/open-source desktop code explorer with interactive dependency graphs and cross-references across C/C++/Java/Python; discontinued by its maker in 2021, no official successor (community forks exist). Structurally the nearest ancestor (desktop, graph-based, cross-language) but static-structure only: no git/temporal mining, no test-discovery, no synthesized reports.
- **GitHub code navigation**: jump-to-definition/find-references in the GitHub web UI, powered by LSIF/SCIP precomputed indexes (originally via Sourcegraph's code-intel extensions). Navigation feature bolted onto a hosting platform, not a standalone comprehension/change-risk tool.
- **Glean**: name collision risk to be aware of, not identical prior art: (1) Meta/Facebook's open-sourced **Glean** (facebookincubator/glean) is a large-scale code-fact indexing system powering jump-to-def/find-refs and LLM code-search agents at Meta scale, conceptually adjacent (fact database over code) but a backend/infrastructure library, not an end-user desktop/CLI tool. (2) **Glean Technologies** is an unrelated enterprise AI search company (est. 2019) with its own "Code Search" feature over connected repos, permission-aware search, not analysis. No clear public trace of a third, distinct "npm's Glean" tool was found in this search; if the creator has a specific npm-internal tool in mind, it appears to be obscure/undocumented publicly, which is flagged here as a research gap rather than asserting nonexistence.
- **aider's "repo map"**: tree-sitter parses the repo into tagged definitions/references, then PageRank-style graph ranking over a file-dependency graph selects the most relevant symbols to fit an LLM's context budget (SQLite-cached AST). Directly relevant prior art for the tree-sitter + symbol-graph pipeline, but it exists purely to feed an AI pair-programming chat session: it doesn't mine git history, discover tests, or produce a standalone human-readable report.
- **Continue**: open-source AI coding-assistant extension; codebase indexing computes local embeddings (transformers.js by default, or a configured model) for `@codebase` similarity search. AI-first, retrieval-for-chat, not evidence-graph-first.
- **Cursor's codebase indexing**: proprietary IDE indexing that chunks and embeds the codebase into a remote vector DB for fast semantic retrieval powering the AI assistant. AI-first, cloud-indexed (not local-first by default), retrieval for chat/completions rather than a structured symbol/call/history graph with citations.
- **CodeScene / Adam Tornhill's temporal-coupling work**: the direct conceptual ancestor of the "mine git history for temporal coupling and evolution" half of this product. CodeScene is the commercial SaaS/on-prem successor to Tornhill's open-source **code-maat**, automating hotspot analysis, temporal coupling, code age, and knowledge-distribution ("crime scene") analytics from VCS logs. Very close prior art for the temporal-coupling engine specifically, but CodeScene is a commercial web dashboard product, not a local-first desktop app + CLI that fuses temporal coupling with a tree-sitter symbol/call graph, test discovery, and cited "blast radius" reports.
- **code-maat**: Tornhill's original open-source CLI (Clojure) that mines VCS logs for coupling/churn/ownership metrics; the direct algorithmic ancestor of the temporal-coupling piece of this product, but purely a metrics-extraction CLI with no symbol/call-graph parsing, no test discovery, and no report synthesis.

**Net positioning**: no existing tool combines (a) tree-sitter symbol/call-graph parsing, (b) git-history temporal-coupling mining à la CodeScene/code-maat, (c) test-discovery, and (d) synthesized, citation-grounded "blast radius / before you change this" reports in one local-first desktop app + shared-engine CLI. The individual techniques all have prior art (Understand/Sourcetrail for graphs, CodeScene/code-maat for temporal coupling, aider for tree-sitter symbol maps); the synthesis and packaging is the gap.

## Candidate Names Considered

- **Karst** (geology): landscape of caves/sinkholes formed by dissolution, revealing hidden underground structure. Rejected at screening (see below).
- **Scarp** (geology): exposed rock face left by a fault/erosion, i.e. structure exposed by a "break." Rejected: direct competitor collision.
- **Outcrop** (geology): bedrock exposed at the surface, "what's revealed when you dig." Rejected: npm taken by an adjacent AI-dev-tool.
- **Talus** (geology): rock debris at the base of a slope. Rejected: squatted across all three registries in unrelated domains.
- **Moraine** (geology): material deposited by a glacier's past movement, evidence of prior motion (temporal/history metaphor). Rejected: taken on all three registries.
- **Sonde**: a probe/sounding instrument (echo/sonar-adjacent). Rejected: taken on all three registries, including an npm collision.
- **Midden** (archaeology): a refuse heap whose layers reveal habitation history (strata metaphor). Rejected: direct competitor collision on crates.io.
- **Barrow** (archaeology): a burial mound. Screened, npm taken by an obscure unrelated package; runner-up.
- **Annalist**: one who writes annals/chronicles; fits temporal/history-mining theme. Screened, npm+crates available but multiple existing devtool-adjacent projects already use the name.
- **Scree** (geology): loose rock debris on a slope. Rejected: taken on all three registries (server-monitoring toolkit).
- **Isochron**: geology dating technique. Considered, dropped pre-screening for being hard to spell/pronounce as a CLI binary.
- **Esker** (geology): a ridge of sediment left by a retreating glacier. Rejected: collides with Esker S.A./Esker Inc., a real, large, publicly traded enterprise-automation company.
- **Drumlin** (geology): a hill shaped by glacial ice, evidence of the direction/history of past movement. Screened; runner-up but collides (lower-severity, different industry) with an existing "Drumlin Security" software vendor.
- **Cairn**: a trail marker/waypoint of stacked stones. Rejected: taken on crates.io.
- **Vestige** (archaeology): a visible trace of something past. Rejected: taken on crates.io.
- **Loam**: soil/roots. Rejected: taken on crates.io.
- **Fissure** (geology): a crack, evocative of "blast radius." Screened; npm/pypi taken.
- **Isobar**: a contour-line metaphor (mapping equal values), evocative of graph/contour visualization. Screened; npm taken, pypi taken.
- **Trench** / **Sounding** / **Augur** / **Waymark** / **Reef** / **Lode** / **Plumbline** / **Tarn**: additional geology/forensics/cartography-adjacent words tested; all rejected, taken on crates.io (several are established Rust CLI/library names already).
- **Coombe**: an old English term for a short valley. Fully available across npm/crates.io/pypi/GitHub, but the metaphor (a valley) doesn't connect to the product's excavation/evidence themes as directly as the winner below; kept as a clean but weaker backup.
- **Borehole** (geology/geotechnical engineering): a narrow shaft drilled into the ground to sample rock strata and history before further excavation or construction. **Selected: see below.**

## Screening Results

For each candidate below: npm and crates.io were checked via direct registry API queries (`registry.npmjs.org`, `crates.io/api/v1/crates/<name>`); PyPI via `pypi.org/pypi/<name>/json`; GitHub via the GitHub REST API (`/users/<name>` for the exact handle and `-app`/`-dev`/`-hq`/`-cli` suffixes, plus `/search/repositories` for existing projects using the word); general web via WebSearch for `"<name>" code developer tool` and `"<name>" trademark company software`. **No USPTO or other formal trademark database was queried. See disclaimer below.**

**Borehole** (selected)
- npm: available (404, no package registered).
- crates.io: available (404, "crate `borehole` does not exist").
- PyPI: available (404).
- GitHub: `github.com/borehole` unclaimed; `borehole-app`, `borehole-dev`, `borehole-hq`, `borehole-cli` all unclaimed. Repo search for "borehole" returns 447 hits, but every result inspected (top 10) is a literal geotechnical/hydrology/petroleum data-science project (groundwater modeling, seismic borehole logging, geospatial toolboxes), zero collisions in the developer-tooling or code-analysis space.
- General web: "borehole" as a software term is exclusively used by literal geotechnical/well-logging software (WinLog, RockWorks Borehole Manager, WellCAD, BoreDM, LOGitEASY), a completely unrelated industry (drilling/geoscience data management), and in none of those cases is "Borehole" itself the standalone product/company brand (the word is used descriptively).
- Notable conflicts: none in developer tools or adjacent software categories. The word is heavily used descriptively in geotechnical engineering software, which lowers rather than raises confusion risk for a dev-tool audience.
- Verdict: **clear**. Full registry trifecta available, clean GitHub namespace, no dev-tool brand collision, and the metaphor (drilling a core sample of strata/history before further excavation) maps directly onto "understand before you change."

**Drumlin** (runner-up)
- npm: available. crates.io: available. PyPI: available.
- GitHub: `github.com/drumlin` is an existing personal user account (not an org/brand); `-app`/`-dev`/`-hq` suffixes all open. Repo search: 76 hits, all unrelated (EnergyPlus/Excel tooling, ArcGIS DEM extraction, misc small personal repos), no dev-tool collisions.
- General web: "Drumlin Security" (PDF DRM/secure-reader vendor, small but real, active LinkedIn/App Store presence) and "Drumlin Partners" (a software-industry M&A advisory firm) both use "Drumlin" as a company name today.
- Notable conflicts: two small, real, differently-scoped software/services companies already trade under "Drumlin." Lower severity than a direct dev-tool competitor, but a genuine existing-brand collision.
- Verdict: **usable but not clean**, kept as a documented fallback.

**Annalist**
- npm: available. crates.io: available (no crate exists).
- PyPI: taken, "Annalist linked data notebook."
- GitHub: account exists as a personal username; `-app`/`-dev`/`-hq` open. Repo search surfaces several existing projects literally named "annalist" in adjacent developer/research-tooling space: `gklyne/annalist` (linked-data notebook, 26★), `noctuid/annalist.el` (Emacs keybinding recorder, 46★), an NLP "ANNALIST" annotation-scoring tool, and a SourceForge listing.
- Verdict: **rejected**. The concept (chronicler of history) fits well, but the name is already meaningfully used multiple times in developer/research tooling; confusion risk in search results is real even without a head-on competitor.

**Barrow**
- npm: taken, an obscure, low-maturity "model adaptor" package (v0.3.0, no real footprint).
- crates.io: available. PyPI: available (404).
- GitHub: `github.com/barrow` is an existing personal account; suffixes open.
- Verdict: **rejected for primary use**. The npm collision is with a near-dead package and could likely be worked around (e.g., publish under a scope), but given a clean alternative exists (Borehole), not worth the residual risk.

**Karst, Scarp, Midden** (rejected: direct competitor collisions)
- **Scarp** is already the exact name of a live crates.io tool described as "Git-native, reviewable project archaeology: what changed, why, and what remains unsettled." This is close enough to the product's own positioning to be a direct naming conflict, not just a taken word.
- **Midden** is already a live crates.io tool: "Resolve, audit, visualize, and clean coding-agent context and state," adjacent AI-dev-tooling space, direct conflict.
- **Karst** is already a live PyPI package: "Code context for AI dev tools, graph-grounded, pack-scoped retrieval over MCP. 60% fewer tokens, audit-grade citations," almost exactly the same problem space (graph-grounded code context, citations). Hard reject.

**Esker** (rejected: real-world brand collision)
- Registries: npm taken (unrelated), crates.io taken, PyPI available, but registry status is moot given the underlying conflict: **Esker S.A. / Esker Inc.** is a real, publicly traded (Euronext), decades-old ($100M+ revenue class) enterprise document/AP automation software company with US offices, a well-established "Esker" trademark in the software space. Too large and too adjacent (enterprise software) a real-world brand to risk.

## Rejected Candidates

- **Karst**, PyPI collision: existing "graph-grounded code context for AI dev tools" package, nearly identical positioning.
- **Scarp**, crates.io collision: existing "git-native project archaeology" tool, nearly identical positioning.
- **Midden**, crates.io collision: existing "coding-agent context" tool, adjacent AI-devtool space.
- **Talus, Moraine, Sonde, Scree, Cairn, Vestige, Loam, Fissure, Isobar, Trench, Sounding, Augur, Waymark, Reef, Lode, Plumbline, Tarn**: each squatted on at least one of npm/crates.io/PyPI by unrelated existing packages (verified via direct registry lookups), several already established Rust crate names.
- **Esker**: collides with Esker S.A./Esker Inc., a real, large, publicly traded enterprise-automation software brand.
- **Isochron**: dropped pre-screening, awkward to spell/pronounce/type as a CLI binary despite a clean concept fit.
- **Annalist**: available on npm/crates.io, but PyPI-taken and already used by multiple existing developer/research-tooling projects (linked-data notebook, Emacs package, NLP annotation tool); confusable in search even without a head-on competitor.
- **Barrow**: clean on crates.io/PyPI/GitHub, but npm-taken by a low-footprint existing package; workable but not fully clean, superseded by Borehole.
- **Coombe**: fully clean across all four surfaces but the "short valley" metaphor is a weaker fit for the product's excavation/evidence positioning than Borehole; kept only as a backup.

## Selected Candidate

**Borehole.** A borehole is a narrow shaft drilled into the ground to pull a core sample and read the strata. That's exactly the move this tool makes on a codebase: drill down through the symbol graph, the call graph, and the git-history layers to produce a cited, evidence-backed read of what's there before anyone builds on top of it or changes it. It reads as a real, ordinary English word (not a strained coined blend), which makes it easy to say, spell, and type as a CLI binary (`borehole`, or a short alias like `bh`), and it has no incumbent meaning in developer tooling: its only existing usage is in an unrelated field (geotechnical/petroleum well-logging software), which is a different-enough domain that a search for "borehole" plus "code" or "developer tool" surfaces nothing conflicting. Preliminary conflict assessment: full registry trifecta (npm, crates.io, PyPI) is unclaimed as of this screening; `github.com/borehole` and the `-app`/`-dev`/`-hq`/`-cli` suffixes are all unclaimed; a GitHub repository-name search for "borehole" returns hundreds of hits, all literal geoscience/engineering projects with zero overlap into code analysis, static analysis, or developer tooling; no company or product in the software industry appears to trade under "Borehole" as a standalone brand. Residual risk is limited to the small possibility that a geotechnical-software company could object to use of a term they use descriptively (low likelihood, given none of them use it as a proper brand name and the industries don't overlap).

This is a preliminary screening based on public web/registry search, not legal advice or formal trademark clearance. No professional trademark search was performed.
