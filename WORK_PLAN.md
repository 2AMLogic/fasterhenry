# Work Plan

Prioritized roadmap of upcoming work, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

## Urgent

*No urgent issues.*

## Ready

Human-approved issues ready for implementation (`loom:issue`) — the M0 set,
seeded at bootstrap:

- **#1**: Geometry model + filament discretization
- **#2**: Partial self/mutual inductance kernels (closed forms + arbitrary-orientation quadrature, SIMD batched)
- **#3**: Mesh assembly + dense complex solve + frequency sweep (blocked by #1, #2)
- **#4**: Validation harness: PyPEEC + Mohan/Greenhouse closed forms on a 2-turn spiral (blocked by #3)
- **#5**: CLI: `.inp` deck reader + JSON in/out (blocked by #3)

## In Progress

*No issues currently being built.*

## Proposed

*No proposed issues.*

## Epics

- **#6**: fasterhenry M0 → release (Phase 1 = M0, issues #1–#5)

## Backlog Balance

| Tier | Count |
|------|-------|
| Tier 1 (goal-advancing) | 4 |
| Tier 2 (goal-supporting) | 1 |
| Tier 3 (maintenance) | 0 |
