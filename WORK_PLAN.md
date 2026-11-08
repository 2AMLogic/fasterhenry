# Work Plan

Prioritized roadmap of upcoming work, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

Released: **0.1.0** and **0.1.1** on crates.io (2026-09-27). Current
milestone: **0.2.0 — drop-in FastHenry replacement** (epic #76). The deck
reader, drop-in CLI mode, compat mode and the `cargo-deny` license gate have
merged; remaining work is the compatibility long tail below.

## Operator Attention: Merge-Risk Holds

_None._ (#262 release 0.2.0 and #280 site skeleton have both merged.)

## Important

- **#166**: Champion merges blocked: branch-protection read returns 403 (repo admin must migrate classic branch protection to a ruleset)

## Ready (`loom:issue`)

_None._

## In Progress

- **#245**: fasterhenry.org gallery and benchmarks pages (`loom:building`; PR #284 awaiting review)
- **#246**: fasterhenry.org validation and FastHenry-users pages (`loom:building`; PR #283 approved, awaiting merge)

(#186 is `loom:blocked`, listed below: PR #189 merged, but it stays open until the operator dispatches `fasthenry-oracle.yml` and records the smoke run.)

## PRs Awaiting Review

- **#284**: feat(site): Gallery and Benchmarks pages generated from committed decks and docs

## Approved (Awaiting Merge)

- **#283**: feat(site): render validation and FastHenry-users pages from docs markdown

## Proposed

_None._

## Blocked / Operator-owned

- **#243**: fasterhenry.org public demo site (umbrella; blocked by #245 and #246, #244 is done)
- **#186**: clean-room FastHenry oracle runner (operator must dispatch the smoke run)
- **#187**: measure FastHenry nonuniform-plane node placement (blocked by #186)
- **#172**: Auditor request for a Rust toolchain (worker spec change needed)
- **#171**: split `fasterhenry-cli/src/inp.rs` into a module directory (architect proposal, blocked)
- **#208**: move the inline test module out of `inp.rs` (hermit proposal, blocked behind #171)
- **#166**: branch-protection ruleset migration (repo admin)
- **#74**: real-deck corpus comparison against FastHenry (operator-run; #69-#72 closed, real corpus rerun still required)
- **#38**: adopt Renovate dependency security policy (open PR)
- **#16**: Champion merge-risk hold digest (tracking issue, not a work item)

## Epics

- **#76**: 0.2.0 — drop-in FastHenry replacement. Children #69–#73, #75 are done; the remaining compatibility follow-ups (#186/#187) are in progress or blocked (#74 remains operator-run).

## Backlog Balance

| Bucket | Count |
|------|-------|
| Very important | 0 |
| Important | 1 (#166) |
| Ready (`loom:issue`) | 0 |
| In Progress | 2 (#245, #246) |
| PRs awaiting review | 1 (#284) |
| Approved PRs awaiting merge | 1 (#283) |
| Curated (proposed) | 0 |
| Blocked | 8 (#16, #74, #171, #172, #186, #187, #208, #243) |
| Operator-gated (`loom:operator`) | 1 (#166) |
| Operator merge-risk holds | 0 |
| Active epics | 1 (#76) |
