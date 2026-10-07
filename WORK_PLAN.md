# Work Plan

Prioritized roadmap of upcoming work, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

Released: **0.1.0** and **0.1.1** on crates.io (2026-09-27). Current
milestone: **0.2.0 — drop-in FastHenry replacement** (epic #76). The deck
reader, drop-in CLI mode, compat mode and the `cargo-deny` license gate have
merged; remaining work is the compatibility long tail below.

## Operator Attention: Merge-Risk Holds

- **#160**: chore(tooling): update Loom 0.19.740 and Repo Skills 0.19.8 (conflicting; touches merge/permission tooling)
- **#159**: docs: repo hygiene pass 2026-10-05 (benchmark figures need human review)

## Ready (`loom:issue`, PR open)

Each has an implementing PR in review:

- **#156**: Deck reader: case-insensitive node names and `.equiv` pseudonyms (PR #164)
- **#155**: Deck reader: `segwid1`/`segwid2` meshed planes (PR #162)
- **#154**: Deck reader: fractional `.freq` ndec, compat sweep edge cases (PR #161)
- **#145**: Compat: clamp a contact cell at or above the rectangle width (PR #152)
- **#144**: Compat: `.units` spellings, missing and repeated `.units` (PR #153)
- **#143**: Compat: `file=NONE` plane with no initial grid is a single root cell (PR #151)

## In Progress

*None claimed (`loom:building`).*

## Blocked / Operator-owned

- **#74**: real-deck corpus comparison against FastHenry (operator-run; head-to-head tooling merged in PR #147)
- **#48**: README fleet burndown embed (blocked on 2AMLogic/2am#1088)
- **#38**: adopt Renovate dependency security policy (open PR)
- **#16**: Champion merge-risk hold digest (tracking issue, not a work item)

## Epics

- **#76**: 0.2.0 — drop-in FastHenry replacement. Children #69–#73, #75 are done; compatibility follow-ups above are in review.

## Backlog Balance

| Tier | Count |
|------|-------|
| Tier 1 (goal-advancing) | 6 (#143–#145, #154–#156) |
| Tier 2 (goal-supporting) | 1 (#74) |
| Tier 3 (maintenance) | 0 |
| Unlabelled / triage | 2 (#16, #48, both blocked) |
