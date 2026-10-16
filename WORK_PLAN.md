# Work Plan

Prioritized roadmap of upcoming work, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

Released: **0.1.0** and **0.1.1** on crates.io (2026-09-27). Current
milestone: **0.2.0 — drop-in FastHenry replacement** (epic #76). The deck
reader, drop-in CLI mode, compat mode and the `cargo-deny` license gate have
merged; remaining work is the compatibility long tail below.

## Operator Attention: Merge-Risk Holds

- **#161**: feat: accept fractional `.freq` ndec, plus compat sweep edge cases (`loom:operator`; implements #154)
- **#189**: feat(tools): add isolated FastHenry oracle workflow and runner (`loom:blocked`; implements #186)

## Important

- **#166**: Champion merges blocked: branch-protection read returns 403 (repo admin must migrate classic branch protection to a ruleset)
- **#156**: Deck reader: case-insensitive node names and `.equiv` pseudonyms (PR #164)
- **#154**: Deck reader: fractional `.freq` ndec, compat sweep edge cases (PR #161)

## Ready (`loom:issue`)

- **#175**: CLI: reject output destinations that alias the input or each other (PR #178 approved, awaiting merge)
- **#156**, **#154**: see Important; each has an implementing PR in review/hold

## In Progress

- **#186**: clean-room FastHenry oracle runner (PR #189)

## Blocked / Operator-owned

- **#187**: measure FastHenry nonuniform-plane node placement (blocked by #186)
- **#172**: Auditor request for a Rust toolchain (worker spec change needed)
- **#166**: branch-protection ruleset migration (repo admin)
- **#74**: real-deck corpus comparison against FastHenry (operator-run)
- **#38**: adopt Renovate dependency security policy (open PR)
- **#16**: Champion merge-risk hold digest (tracking issue, not a work item)

## Epics

- **#76**: 0.2.0 — drop-in FastHenry replacement. Children #69–#73, #75 are done; compatibility follow-ups above are in review (#74 remains operator-run).

## Backlog Balance

| Bucket | Count |
|------|-------|
| Very important | 0 |
| Important | 3 (#154, #156, #166) |
| Ready (`loom:issue`) | 3 (#154, #156, #175) |
| In Progress | 1 (#186) |
| Blocked | 4 (#16, #166, #172, #187) |
| Active epics | 1 (#76) |
