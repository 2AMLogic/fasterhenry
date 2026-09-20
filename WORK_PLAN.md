# Work Plan

Prioritized roadmap of upcoming work, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

## Urgent

*No urgent issues.*

## Ready

M0 is complete (merged 2026-09-20/21 overnight session). The M1
FastHenry-parity set, filed and ready in implementation order:

- **#21**: binary `Zc.mat` (MAT v4) + SPICE subcircuit export
- **#22**: ground planes (`G`) with holes
- **#23**: skin-depth-adaptive filament subdivision
- **#24**: precorrected-FFT acceleration
- **#25**: coupling truncation (`.couples`-style)

## In Progress

*No issues currently being built.*

## Proposed

*No proposed issues.*

## Epics

- **#6**: fasterhenry M0 → release (Phase 1 = M0). Done: #1 geometry (PR #11,
  #17), #2 kernels (PR #13), #3 mesh/solve (PR #18), #5 CLI (PR #19),
  #8 publish-readiness (PR #10). Remaining for M0: #4; then #9
  (operator-gated crates.io publish — unblocked now that #8 landed).

## Backlog Balance

| Tier | Count |
|------|-------|
| Tier 1 (goal-advancing) | 2 (#4, #6) |
| Tier 2 (goal-supporting) | 1 (#9) |
| Tier 3 (maintenance) | 3 (#14, #15, #16 — blocked follow-ups) |
