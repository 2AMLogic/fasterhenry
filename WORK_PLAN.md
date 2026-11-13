# Work Plan

Prioritized roadmap of upcoming work, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

Released: **0.1.0** and **0.1.1** on crates.io (2026-09-27). Current
milestone: **0.2.0 — drop-in FastHenry replacement** (epic #76). The deck
reader, drop-in CLI mode, compat mode and the `cargo-deny` license gate have
merged, and the 0.2.0 release PR (#262) has merged. Remaining work is the
public demo site umbrella (#243; the gallery and benchmarks pages in #245 have merged) and the compatibility long tail below.

## Operator Attention: Merge-Risk Holds

_None._

## Important

- **#166**: Champion merges blocked: branch-protection read returns 403 (repo admin must migrate classic branch protection to a ruleset)

## Ready (`loom:issue`)

_None._

## In Progress

_None._

(#186 is `loom:blocked`, listed below: PR #189 merged, but it stays open until the operator dispatches `fasthenry-oracle.yml` and records the smoke run.)

## PRs Awaiting Review

_None._

## Proposed

- **#295**: Conductivity models: temperature coefficient and nanoscale size effects (Fuchs–Sondheimer, Mayadas–Shatzkes) *(curated; operator-gated)*
- **#340**: Shared blocks, material schema, structure file and two interchange formats: proposals touching fasterhenry *(awaiting curation; asks for answers on units, structure file and material library)*
- **#344**: Deduplicate hand-copied test and bench fixtures in fasterhenry (serpentine, meander, spiral, coupled structure) *(hermit proposal; awaiting approval)*

## Blocked / Operator-owned

- **#243**: fasterhenry.org public demo site (umbrella; #244, #245 and #246 done, awaiting close-out)
- **#311**: site landing page with proof figures, an example and a Get started page (operator-gated; last open child of #303)
- **#186**: clean-room FastHenry oracle runner (operator must dispatch the smoke run)
- **#187**: measure FastHenry nonuniform-plane node placement (blocked by #186)
- **#172**: Auditor request for a Rust toolchain (worker spec change needed)
- **#171**: split `fasterhenry-cli/src/inp.rs` into a module directory (architect proposal, blocked)
- **#317**: remove `site/gen_hero.py` (duplicates `tools/gallery_figures.py`; blocked by #311)
- **#208**: move the inline test module out of `inp.rs` (hermit proposal, blocked behind #171)
- **#166**: branch-protection ruleset migration (repo admin)
- **#74**: real-deck corpus comparison against FastHenry (operator-run; #69-#72 closed, real corpus rerun still required)
- **#16**: Champion merge-risk hold digest (tracking issue, not a work item)

## Epics

- **#303**: fasterhenry.org redesign: tables that fit, readable pages, a real landing page (8 of 9 children done; #311 landing page open, operator-gated).
- **#76**: 0.2.0 — drop-in FastHenry replacement. Children #69–#73, #75 are done; the remaining compatibility follow-ups (#186/#187) are in progress or blocked (#74 remains operator-run).

## Backlog Balance

| Bucket | Count |
|------|-------|
| Very important | 0 |
| Important | 1 (#166) |
| Ready (`loom:issue`) | 0 |
| In Progress | 0 |
| PRs awaiting review | 0 |
| Curated (proposed) | 3 (#243, #295, #311; all operator-gated) |
| Awaiting curation | 1 (#340) |
| Hermit proposals (unblocked) | 1 (#344) |
| Blocked | 8 (#16, #74, #171, #172, #186, #187, #208, #317) |
| Operator-gated (`loom:operator`) | 4 (#166, #243, #295, #311) |
| Operator merge-risk holds | 0 |
| Active epics | 2 (#76, #303) |
