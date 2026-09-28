# Work Plan

Prioritized roadmap of upcoming work, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

## Urgent

*No urgent issues.*

## Ready

Released: **0.1.0** and **0.1.1** on crates.io (2026-09-27). Current
milestone: **0.2.0 — drop-in FastHenry replacement** (epic #76). #69
(FastHenry `G` ground-plane syntax) is merged. Remaining, in priority
order:

- **#73**: drop-in CLI mode (`fasterhenry deck.inp` writes `Zc.mat`)
- **#75**: `cargo-deny` license allowlist gate in CI
- **#80**: plane hole/contact shapes the plane model cannot represent yet
- **#83** (triage): compatibility mode for third-party decks (implicit
  first-line title)

## In Progress

- **#70** — `rho=` and the full `.units` list (PR #82: changes requested, merge conflict)
- **#71** — `rw`/`rh` filament ratios, `wx`/`wy`/`wz` width direction (PR #85: in review)
- **#72** — format audit + compatibility table (PR #84: changes requested, merge conflict)

## Proposed

*No proposed issues.*

## Epics

- **#76**: 0.2.0 — drop-in FastHenry replacement. Done: #69. In progress:
  #70, #71, #72. Next: #73, #75. Operator-run: #74 (real-deck corpus,
  blocked by #69–#72), #35 (required status checks).
- **#6**: M0 → release. All four phases have shipped (dense core,
  physics completeness, pFFT acceleration, crates.io release #9); open
  only as an operator-owned tracking epic.

## Backlog Balance

| Tier | Count |
|------|-------|
| Tier 1 (goal-advancing) | 6 (#6, #70, #71, #72, #73, #76) |
| Tier 2 (goal-supporting) | 2 (#74, #75) |
| Tier 3 (maintenance) | 0 |
| Unlabelled / triage | 4 (#16, #48 blocked; #80, #83) |
