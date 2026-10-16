# Work Plan

Prioritized roadmap of upcoming work, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

Released: **0.1.0** and **0.1.1** on crates.io (2026-09-27). Current
milestone: **0.2.0 — drop-in FastHenry replacement** (epic #76). The deck
reader, drop-in CLI mode, compat mode and the `cargo-deny` license gate have
merged; remaining work is the compatibility long tail below.

## Operator Attention: Merge-Risk Holds

- **#161**: feat: accept fractional `.freq` ndec, plus compat sweep edge cases (`loom:pr` + `loom:blocked`; implements #154)
- **#189**: feat(tools): add isolated FastHenry oracle workflow and runner (`loom:pr` + `loom:blocked`; implements #186)

## Important

- **#166**: Champion merges blocked: branch-protection read returns 403 (repo admin must migrate classic branch protection to a ruleset)
- **#154**: Deck reader: fractional `.freq` ndec, compat sweep edge cases (PR #161)

## Ready (`loom:issue`)

- **#154**: see Important; implementing PR #161 is under hold

## In Progress

- **#186**: clean-room FastHenry oracle runner (PR #189)

## Blocked / Operator-owned

- **#187**: measure FastHenry nonuniform-plane node placement (blocked by #186)
- **#172**: Auditor request for a Rust toolchain (worker spec change needed)
- **#171**: split `fasterhenry-cli/src/inp.rs` into a module directory (architect proposal, blocked)
- **#166**: branch-protection ruleset migration (repo admin)
- **#74**: real-deck corpus comparison against FastHenry (operator-run; #69-#72 closed, real corpus rerun still required)
- **#38**: adopt Renovate dependency security policy (open PR)
- **#16**: Champion merge-risk hold digest (tracking issue, not a work item)

## Epics

- **#76**: 0.2.0 — drop-in FastHenry replacement. Children #69–#73, #75 are done; the remaining compatibility follow-ups (#154, #186/#187) are in review or blocked (#74 remains operator-run).

## Backlog Balance

| Bucket | Count |
|------|-------|
| Very important | 0 |
| Important | 2 (#154, #166) |
| Ready (`loom:issue`) | 1 (#154) |
| In Progress | 1 (#186) |
| Blocked | 6 (#16, #74, #166, #171, #172, #187) |
| Active epics | 1 (#76) |
