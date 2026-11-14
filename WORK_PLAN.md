# Work Plan

Prioritized roadmap of upcoming work, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

Released: **0.1.0** and **0.1.1** on crates.io (2026-09-27). Current milestone:
**0.2.0 — drop-in FastHenry replacement** (epic #76). The deck reader, drop-in
CLI mode, compat mode and the `cargo-deny` license gate have merged, and the
0.2.0 release PR (#262) has merged — but there is no `v0.2.0` tag and crates.io
still lists 0.1.1, so **0.2.0 is not published**. The epic's one open child is
the operator-run real-deck corpus comparison (#74).

Everything between the markers below is generated from current label state on
each Guide tick; edits there are overwritten. For *why* an item is held and what
it is waiting on, see the "Blocked and waiting" table in
[`docs/ROADMAP.md`](docs/ROADMAP.md) — that file is the hand-maintained
direction behind this queue, and it is where operator-owned items are explained.

<!-- guide:plan-body:start -->
## Operator Attention: Merge-Risk-Hold Pileup

Judge-approved PRs under a hold (`loom:blocked` or `loom:operator`) — only a human merge decision is missing.

_None._

## Very Important

`loom:very-important` (one counts per workspace); every stage takes these first.

_None._

## Important

`loom:important`; taken before unlabelled work.

- **#311**: site: landing page that says what it does, with proof figures, an example and a Get started page
- **#366**: docs: two mislabelled figures in the high-frequency refinement section (#353 follow-up)

## Ready

Human-approved issues ready for implementation (`loom:issue`).

- **#368**: Share the sweep preamble between run_reporting_with and run_with_currents: 13 byte-identical lines, one copy untested

## In Progress

Issues currently being built (`loom:building`).

- **#351**: Document the JSON problem input format (docs/input-format.md)
- **#352**: Add runnable library examples under fasterhenry/examples/
- **#362**: Collapse the five hand-rolled gate-ran assertions in ci.yml and drop the two SKIPPED greps the marker counts subsume
- **#364**: Deduplicate the pFFT random-filament generator: three verbatim copies of cloud/random_set
- **#369**: Deduplicate the PyPEEC validation fixtures: every gated geometry is defined twice, in two languages, unchecked

## PRs Awaiting Review

PRs waiting on Judge (`loom:review-requested`).

- **#367**: ci: assert every validation gate ran through one helper, drop the SKIPPED greps
- **#370**: refactor(bench): share the pFFT random-filament generator from benches/support.rs
- **#371**: test(validation): declare each PyPEEC reference's geometry and check it

## Approved (Awaiting Merge)

PRs that passed review and are queued for Champion auto-merge (`loom:pr`).

- **#360**: docs: document the JSON problem input format
- **#361**: docs: add runnable dense and iterative examples under fasterhenry/examples/

## Proposed

Issues carrying `loom:curated`.

- **#166**: Champion merges blocked: branch-protection read returns 403 (Resource not accessible by integration) *(curated)*
- **#243**: fasterhenry.org: public demo site (landing, gallery, benchmarks, validation, FastHenry compat) on Cloudflare Pages *(curated)*
- **#295**: Conductivity models: temperature coefficient and nanoscale size effects (Fuchs–Sondheimer, Mayadas–Shatzkes) *(curated)*
- **#311**: site: landing page that says what it does, with proof figures, an example and a Get started page *(curated)*
- **#351**: Document the JSON problem input format (docs/input-format.md) *(curated)*
- **#352**: Add runnable library examples under fasterhenry/examples/ *(curated)*
- **#366**: docs: two mislabelled figures in the high-frequency refinement section (#353 follow-up) *(curated)*

## Proposed (Architect / Hermit)

- **#171**: Refactor: split the 8.5k-line fasterhenry-cli/src/inp.rs into an inp/ module directory *(architect)*
- **#208**: Split inp.rs: move 5k-line inline test module into a child file *(hermit)*
- **#317**: Remove site/gen_hero.py: duplicates tools/gallery_figures.py for the spiral deck *(hermit)*
- **#359**: Remove ten never-called public accessors from the fasterhenry library API *(hermit)*

## Epics

- **#76**: Epic: 0.2.0 — drop-in FastHenry replacement (deck compatibility, drop-in CLI, license gate)
- **#303**: fasterhenry.org redesign: tables that fit, readable pages, a real landing page

## Backlog Balance

| Bucket | Count |
|------|-------|
| Operator merge-risk holds | 0 |
| Very important | 0 |
| Important | 2 |
| Ready (`loom:issue`) | 1 |
| In Progress (`loom:building`) | 5 |
| PRs awaiting review | 3 |
| Approved PRs awaiting merge | 2 |
| Curated | 7 |
| Architect / Hermit proposals | 4 |
| Active epics | 2 |
<!-- guide:plan-body:end -->
