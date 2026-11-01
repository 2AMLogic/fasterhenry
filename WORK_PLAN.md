# Work Plan

Prioritized roadmap of upcoming work, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

Released: **0.1.0** and **0.1.1** on crates.io (2026-09-27). Current
milestone: **0.2.0 — drop-in FastHenry replacement** (epic #76). The deck
reader, drop-in CLI mode, compat mode and the `cargo-deny` license gate have
merged; remaining work is the compatibility long tail below.

## Operator Attention: Merge-Risk Holds

- **#238**: Loom resync 0.19.1245 → 0.19.1246 (Judge-approved, CI green, merge state clean; held by Champion because it touches merge-gate test scripts — needs a human merge decision)
- **#241**: Loom resync 0.19.1246 → 0.19.1247 (Judge-approved, CI green, merge state clean; same merge-gate hold as #238, and supersedes it if #238 is closed unmerged)

## Important

- **#166**: Champion merges blocked: branch-protection read returns 403 (repo admin must migrate classic branch protection to a ruleset)

## Ready (`loom:issue`)

_None._

## In Progress

_None._ (#186 is `loom:blocked`, listed below: PR #189 merged, but it stays open until the operator dispatches `fasthenry-oracle.yml` and records the smoke run.)

## Blocked / Operator-owned

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
| In Progress | 0 |
| Blocked | 8 (#16, #74, #166, #171, #172, #186, #187, #208) |
| Operator merge-risk holds | 2 (#238, #241) |
| Active epics | 1 (#76) |
