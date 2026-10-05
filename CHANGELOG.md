# Changelog

All notable changes to the `fasterhenry` library and the `fasterhenry-cli`
binary crate are recorded here. Both crates share one version number and this
one file (each crate ships a copy in its published package).

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While the version is `0.x`, a minor release (`0.1` → `0.2`) may make
breaking changes to the API or the command-line interface; patch releases
(`0.1.0` → `0.1.1`) stay compatible.

## [Unreleased]

### Added

- Deck reader: `relx=` / `rely=` / `relz=` on a corner-point `G` statement
  (issue #146), the documented offset (User's Guide §1.3.9), in every mode.
  It is added to every node-reference, hole and contact coordinate on the
  statement but not to the corner points, applies wherever it is written,
  and a repeated key's last value wins. Previously rejected by name. Under
  `--fasthenry-compat`, one empty coordinate field in a node reference
  (`N1 (, 2, 0)`) now reads as 0 (before the offset) with a line-numbered
  warning; natively it is still an error.
- Deck reader: under `--fasthenry-compat`, a segment or ground plane with
  no conductivity on its line or in `.default` defaults to copper,
  5.8e7 S/m as a physical value whatever the `.units`, with one
  line-numbered warning per statement (issue #142). Natively it is still a
  line-numbered error.
- Deck reader: a line-numbered **warning** for a corner-point `G`
  statement's `hole point`, `hole circle`, `hole rect`, `contact rect` or
  `contact decay_rect` clause that lies **wholly outside** the plane's
  footprint (issue #105). Such a clause was silently accepted and changed
  nothing — the typical result of a coordinate typo or a wrong `.units`
  scale. It is still accepted, and the mesh is unchanged, but
  `fasterhenry` now prints `warning: line N: '<clause>' … lies wholly
  outside ground plane '<name>' …` on stderr, once per offending clause,
  naming the clause's own physical (continuation) line; stdout is
  unchanged. The test is intersection with the closed footprint: a circle
  or rectangle that overhangs an edge, reaches over a corner, or merely
  touches the boundary does not warn (a contact rectangle that only
  touches the boundary does, since #134), and neither does an in-footprint
  clause that happens to change nothing. The stricter clauses keep their
  errors (`contact point` / `line` / `trace` off the plane; a `contact
  equiv_rect` / `contact connection` whose node is off the plane or that
  covers no live cell). Library: `inp::ParseWarning`,
  `inp::parse_reporting`, `inp::parse_with_options_reporting`,
  `read_inputs_reporting` and `read_inputs_reporting_with` return the
  warnings; `parse`, `parse_with_options`, `read_inputs` and
  `read_inputs_with` are unchanged and discard them, and nothing in the
  library prints. See `docs/fasthenry-compat.md`, "Decision: a hole or
  contact rectangle off its plane is accepted with a warning".

- Deck reader: the **documented** `contact rect (x, y, z, xwidth, ywidth,
  xcell, ycell)` spelling on a corner-point `G` ground-plane statement
  (issue #95, found while implementing `contact decay_rect` for #80). Until
  now this reader spelled `contact rect` only as two opposite corners,
  mirroring `hole rect`; the public format documents it the way every other
  `contact` shape is documented — the rectangle's **centre**, its **full
  widths** about that centre, and the largest **cell** wanted inside it — so
  a genuine third-party deck was rejected for its value count, and a
  six-value clause this reader did accept meant something other than what
  that spelling means. **Both are now read, told apart by value count
  alone**: seven values are the documented form, six keep the corner
  meaning as this reader's own extension, and any other count is a
  line-numbered error naming both. No deck that parsed before changes
  meaning. The seven-value form is exactly `contact decay_rect` without its
  two outward limits — `contact rect (x, y, z, xw, yw, xc, yc)` is
  `contact decay_rect (x, y, z, xw, yw, xc, yc, -1, -1)` — so it inherits
  that clause's per-axis cell count, its documented decay law
  `1/(1 − cell/width)` back to the plane's background cell, and its
  requirement that `cell` be smaller than `width`; and because the corner
  form's default 2 × 2 cells at ratio 2 is the seven-value form at
  `cell = width/2`, `contact rect (4, 2, 0, 6, 4, 0)` and `contact rect (5,
  3, 0, 2, 2, 1, 1)` are the same region. What the documented form buys is
  the cell size itself, per axis, which the corner form cannot state. Being
  exactly `contact decay_rect`, the documented form's widths and cells are
  read in the **plane's own** coordinate system, as `decay_rect`'s are
  (issue #118); the six-value corner form's two corners stay global
  coordinates, unaffected. Why accepting both beat migrating to the
  documented form alone, or documenting the divergence permanently, is
  written out in `docs/fasthenry-compat.md` § *Decision: `contact rect`
  reads both spellings, told apart by value count*.
- Deck reader: the **initial-grid** clauses on a corner-point `G`
  ground-plane statement — `contact initial_grid (n1, n2)` and
  `contact initial_mesh_grid (n1, n2)` (issue #113, follow-up to #101, which
  left both rejected rather than guess their row/column convention). The
  convention is now read out of the public description rather than guessed:
  `n1` counts cells along `p1 → p2` and `n2` along `p2 → p3`, which is the
  same pair `seg1`/`seg2` set — the equivalence that description states
  outright, and the only reading under which its own worked example comes
  out square-celled (8500/34 = 13500/54 = 250). The one sentence of it that
  reads the other way, and why it does not overturn those two, is written
  out in `docs/fasthenry-compat.md` § *Decision: `(n1, n2)` counts
  `p1 → p2` then `p2 → p3`*. `initial_mesh_grid` additionally punches the
  documented checkerboard — every cell whose two indices, numbered from 1 at
  the plane's own origin (its `p1` corner), are both even — as one
  `fasterhenry::plane::Hole::Rect` per holed cell, the whole square rather
  than a point at its centre, so a later `contact` clause refining that
  region cannot shrink the hole. Because the initial grid and `seg1`/`seg2`
  are one statement, a plane giving both (in either order), or two initial
  grids, is a line-numbered error rather than a race between them; no deck
  that parsed before changes meaning.
- Deck reader: the `contact trace (x0, y0, z0, x1, y1, z1, trace_width,
  scale_factor)` clause on a corner-point `G` ground-plane statement, for a
  trace **parallel to x or y** (issue #110, follow-up to #100). It refines
  the plane finely across the trace's projection but not along it, and is
  exactly the five `contact line`s the public memo *Nonuniformly
  Discretized Reference Planes in FastHenry 3.0* (M. Kamon, 1996) spells
  out in its worked example: at the trace and `±trace_width/2` either side,
  cells `trace_width/2` across; at `±3·trace_width/2`, cells `trace_width`
  across; along the trace, cells as long as the trace. `scale_factor` has
  no effect on an axis-aligned trace (the memo says so) and is only checked
  to be positive. The trace's own ends are checked against the plane's
  slab and footprint on the statement's own line; its side lines are
  clipped to the plane, so a trace along the plane's edge is accepted. A
  zero-length trace is a line-numbered error naming `contact point`. A
  trace **not** parallel to x or y stays rejected by name, now as a
  decision rather than deferred work: the memo's two statements of how
  `scale_factor` magnifies a diagonal trace's cells disagree past 45°, and
  it never says where a diagonal trace's side lines go, so the cell size
  is not determined — the error names `contact line` instead (see
  `docs/fasthenry-compat.md`, "Decision: a diagonal `contact trace` is
  rejected").
- Deck reader / library: named **contact areas** on a corner-point `G`
  ground-plane statement — `contact equiv_rect N<name> (x, y, z, xwidth,
  ywidth)` and its documented shorthand `contact connection N<name> (x, y,
  z, xwidth, ywidth, ratio)` (issue #101, follow-up to #80). The rectangle
  (centre and full widths, the spelling `decay_rect` uses) is tied to the
  one node the clause names, so a deck can `.equiv` an external node — or
  point `.external` — onto a landing *pad* rather than onto the single
  nearest cell centre, which is a materially different thing for
  resistance. `connection` expands to exactly an `equiv_rect` plus a
  `decay_rect` over the same rectangle with cells `xwidth/ratio`,
  `ywidth/ratio` and no decay limit; writing the two out by hand gives an
  identical deck, and `ratio` must be > 1. The clause's node name (between
  the shape word and the value list — a form the plane-statement scanner
  did not previously allow) follows the in-plane-node rule, `N`-prefixed;
  a name on a clause that names no node, and a rectangle catching no live
  cell centre, are both line-numbered errors rather than silently dropped.
  An `.equiv` set holding both a contact area and a plain in-plane node of
  the same plane lands on the *area*.
- Library: `fasterhenry::plane::Equipotential` and
  `GroundPlane::equipotentials` — a rectangular patch of plane tied to one
  node, with `GroundPlane::equipotential_node` reporting the node a patch
  produced (issue #101). Every live cell centre inside the rectangle
  (boundary included) shares a single node, the bars that ran between those
  cells are not built, and the bars crossing the patch's boundary end on
  that node, which sits at the mean of the cell centres it ties; two
  rectangles tying a cell in common merge into one equipotential, since two
  overlapping perfect conductors are one conductor. The consequence is
  stated rather than left to be inferred: the model is exact for a patch
  covering one cell, and for a larger patch each entering bar reaches the
  tie through metal the mesh still treats as ordinary plane rather than as
  the perfect conductor the patch is — an over-, never under-, estimate of
  the contact's own resistance, and one that does not grow as the mesh is
  refined, unlike the constriction of a landing on a single cell centre.
  `an_equipotential_pad_lowers_resistance_and_stops_the_mesh_setting_it`
  (`fasterhenry/tests/plane_validation.rs`) measures both halves of that on
  one mesh. **Breaking**: `GroundPlane` gains a public
  `equipotentials: Vec<Equipotential>` field, so exhaustive struct literals
  need one more line (`equipotentials: Vec::new()` reproduces the previous
  behaviour exactly — a plane with none meshes, numbers its nodes and
  builds its bars precisely as before).
- CLI: a drop-in invocation form, `fasterhenry <deck.inp | problem.json>`
  with no subcommand (issue #73, a phase of the 0.2.0 drop-in-replacement
  epic), for scripts written against FastHenry. It takes exactly the options
  `run` takes — the input may precede or follow them — and differs only in
  its default output: it writes `Zc.mat` in the working directory even
  without `--zc-mat`, where `run` still writes one only when asked.
  `--zc-mat <path>` replaces that default rather than adding a second file,
  and the JSON result still goes to stdout (or `--json <path>`). `fasterhenry
  run …` is unchanged, so existing 0.1 invocations keep working; the help is
  now the option list under `fasterhenry --help`, with `fasterhenry help`
  keeping the subcommand listing. Nothing a command line names is silently
  ignored on either form: an option or extra argument this CLI does not
  define is an error naming it, which is the whole of the policy — FastHenry's
  own historical flags are deliberately *not* enumerated, since that would
  mean reading documentation the clean-room rule in `CONTRIBUTING.md` keeps
  out of this project. One naming corner: the first argument is read as a
  subcommand when it is exactly one (`run`, `help`), so a deck named `run`
  needs `fasterhenry run run` or a qualified path (`./run`).
- Deck reader: FastHenry's ground-plane (`G`) statement syntax (issue #69,
  phase 1 of the 0.2.0 deck-compatibility epic). A plane may now be
  declared in the documented corner-point form — three corner points,
  `thick=`, `seg1=`/`seg2=`, `sigma=`, `nhinc=`, with in-plane nodes
  (`N<name> (x, y, z)`), `hole rect (…)` and `contact rect (…)` clauses on
  the statement itself — as well as in this project's existing
  `G<name> x1 y1 z1 x2 y2 z2 t` shorthand with `.hole` / `.contact`. The
  two grammars are told apart by the shape of the first token after the
  plane name, never by a deck-wide mode, so one deck may mix them and
  `.hole`, `.contact`, `.equiv` and endpoint landing behave identically
  whichever form declared the plane. The corner-point form names the
  plane's *mid-thickness* surface where the shorthand names its top, and
  `seg1`/`seg2` become the background cell counts of this engine's own
  cell-centre PEEC mesh. `.equiv` now joins any number of nodes, and an
  in-plane node makes its whole joined set land on that plane whichever
  side of the directive named it first — so wiring a via into a plane does
  not depend on argument order. Joining in-plane nodes of two *different*
  planes is rejected rather than silently attaching to one.
  `nhinc=` is also accepted on the shorthand form.
  Every remaining documented plane parameter is rejected by name, with the
  statement's line number and the alternative: `rh`, `segwid1`/`segwid2`,
  `relx`/`rely`/`relz`, `file`, and every hole or contact shape other than `rect` (`point`, `circle`,
  `decay_rect`, `line`, `trace`, `connection`, the `initial_*`/`equiv_*`
  forms and the user-defined `user1…user7`) — nothing on a `G` statement is
  silently ignored, and representing those shapes is tracked in #80.
  (`contact decay_rect`, `hole point`/`hole circle`, `contact
  point`/`contact line`, `contact equiv_rect`/`contact connection` and the
  `contact initial_grid`/`initial_mesh_grid` pair have since been
  implemented — see their own entries below; the rest are still rejected,
  tracked in #99, #109 and #110.)
  `fasterhenry-cli/tests/data/plane_fasthenry.inp` and
  `plane_extension.inp` are the same self-authored plane problem in the two
  syntaxes, and a new test requires them to produce the same geometry and
  the same `Z(ω)`.
- Deck reader: `rho=` (resistivity, per deck unit) is accepted on
  `.default`, `E` segments and extension-form `G` ground planes as the
  reciprocal of `sigma=` (issue #70); `rho` must be positive, and giving
  both `sigma=` and `rho=` on one line is a line-numbered error. The
  extension form `G<name> x1 y1 z1 x2 y2 z2 t` now also takes a per-plane
  `sigma=` overriding `.default`, alongside its `nx=`/`ny=`/`nhinc=`.
- Deck reader: `rho=` is accepted directly on the corner-point `G`
  ground-plane statement too — the syntax real FastHenry decks use — so the
  whole deck format now takes the conductivity either way wherever it takes
  it at all (issue #88, follow-up to #70). It converts to a conductivity
  exactly as elsewhere (`1/rho` per deck unit), so a corner-point plane with
  `rho=0.5` parses to the same deck as one with `sigma=2`; naming both
  `sigma=` and `rho=` on one corner-point statement — continuation lines
  included — is the same line-numbered error as everywhere else, and naming
  neither still falls back to the `.default` conductivity.
- Deck reader: the `contact decay_rect` clause on a corner-point `G`
  ground-plane statement (issue #80, follow-up to #69). Its nine documented
  values — the rectangle's centre `(x, y, z)`, its full widths
  `xwidth`/`ywidth` about that centre, the largest cell wanted inside it
  (`xcell`/`ycell`) and the largest cell the outward decay may grow to
  (`xmaxcell`/`ymaxcell`, negative for no limit) — map onto a
  `fasterhenry::plane::ContactRegion` per axis: `ceil(width / cell)` fine
  cells, and the documented decay law `1/(1 − cell/width)` as that axis's
  outward growth ratio. `cell` must be smaller than `width` (the
  documentation's own `r0 < 1`), and because this engine's grading levels
  off at the plane's *background* cell, a positive `maxcell` finer than that
  cell is rejected by name (raise `seg1`/`seg2`) rather than silently
  yielding a coarser mesh; one at or above it never binds.
  `fasterhenry-cli/tests/data/plane_decay_rect.inp` and
  `plane_decay_extension.inp` are the same self-authored problem written as
  a `decay_rect` clause and as a `.contact` directive, and a new test
  requires them to produce the same geometry and the same `Z(ω)`. Every
  other hole and contact shape keeps its line-numbered rejection, now
  tracked one issue per model change it needs: `hole point`/`hole circle`
  (has since been implemented — see #98's own entry below), #99
  (`hole user1…user7`, since decided as a permanent rejection — see its
  entry under "Changed"), #100 (`contact point`, `line`, `circle`, `trace`;
  `point` and `line` have since been implemented — see #100's own entry
  below, with `circle` and `trace` moved to #109 and #110; `trace` along x
  or y has since been implemented — see #110's own entry above)
  and #101 (`contact equiv_rect`, `connection`, `initial_grid`,
  `initial_mesh_grid`; `equiv_rect` and `connection` have since been
  implemented — see #101's own entry above — with the two `initial_*` forms
  left rejected and moved to #113, which has since implemented them too).
- Library: `fasterhenry::plane::Hole` is now an enum (`Rect`, `Point`,
  `Circle`) instead of a rectangle-only struct, and the deck reader accepts
  the corner-point `G` statement's `hole point (x, y, z)` and
  `hole circle (x, y, z, r)` clauses (issue #98, follow-up to #80). A point
  removes exactly the one cell whose own extent — edges included — contains
  it; a point landing exactly on a shared cell edge or corner is the
  documented tie and removes every cell touching it, rather than guessing a
  single winner. A circle removes every cell whose centre lies at or inside
  its radius (a centre exactly on the circle counts — a closed boundary,
  unlike `hole rect`'s strict-interior test, since there is no prior
  rectangle behaviour to match). Both are checked against the plane's slab
  on their own `z`, like every other hole/contact clause. This is a breaking
  change to `fasterhenry::plane::Hole`'s public shape: existing callers
  constructing `Hole { lo, hi }` now write `Hole::Rect { lo, hi }`. The enum
  is `#[non_exhaustive]` from the start, so a future shape variant will not
  need another breaking release the way this one did. (The variant this
  anticipated for `hole user1`…`user7` is not being added — see #99's entry
  under "Changed".)
- Deck reader: the `contact point (x, y, z, xcell, ycell)` and `contact line
  (x0, y0, z0, x1, y1, z1, xcell, ycell)` clauses on a corner-point `G`
  ground-plane statement (issue #100, follow-up to #80): every cell holding
  the point, or crossed by the line, is no larger than `xcell` × `ycell`.
  Each maps onto one `fasterhenry::plane::ContactRegion` — the locus's
  bounding box padded by half a requested cell on every side, cut into the
  fewest cells no larger than that cell, grading back to the background at
  the reader's default ratio 2 — so a point is exactly one `xcell × ycell`
  cell centred on it (a via landing there snaps onto its centre) and a
  zero-length line is that point. A requested cell at or above the plane's
  background cell is already met and is clamped there rather than
  coarsening the mesh. A diagonal line refines its whole padded bounding
  box: on this engine's tensor-product mesh any refinement covering the
  line has those same per-axis bands, so that is the stated cost (about
  `(Lx/xcell)·(Ly/ycell)` fine cells rather than `Lx/xcell + Ly/ycell`),
  not an approximation. Both ends are checked against the plane's slab and
  footprint on the statement's own line. `contact circle` and `contact
  trace` keep their line-numbered rejection, now tracked in #109 and #110:
  the second's cell-size rule is not yet pinned from the public
  documentation and is not guessed, and the first turned out not to be a
  documented shape at all — see its entry under Changed below. (`contact
  trace` along x or y has since been implemented — see #110's entry above.)
- Library: `ContactRegion::graded_per_axis`, a contact region whose outward
  decay ratio is chosen per axis — what an anisotropically refined region
  needs, and what `contact decay_rect` derives from the deck (issue #80).
- Deck reader: `.units` accepts the full documented list — `km`, `m`, `cm`,
  `mm`, `um`, `in`, `mils` (`mil` kept as a synonym) — case-insensitively.
- `docs/fasthenry-compat.md`: a field-by-field compatibility table auditing
  the deck reader against the public FastHenry `.inp` input-format
  description — every directive, supported / differs / deferred, with the
  reason (issue #72). Linked from `fasterhenry-cli/README.md`. Tests now
  pin the multi-node `.equiv a b c …` join and its self-alias rejection.
- Deck reader: segment filament ratios and width direction (issue #71).
  `E` lines and `.default` now accept `rw`/`rh` — the ratio of adjacent
  filament extents across the width and the height, each axis
  independently, coarsening from the surfaces inward (`nwinc=5 rw=2` cuts
  the width 1:2:4:2:1) — and `wx`/`wy`/`wz`, a vector along the
  cross-section's width that orients the segment (a flat bar turned on
  edge). An omitted ratio keeps the uniform grid, so existing decks solve
  exactly as before. A ratio that is not a number ≥ 1, a zero width
  direction, and one parallel to the segment are errors on their own line;
  a segment the geometry rejects is now reported on its `E` line instead
  of line 0.
- Library: `discretize_graded_per_axis`, the surface-graded grid with an
  independent ratio along each cross-section axis (`discretize_graded` is
  its equal-ratio case, unchanged), and `Discretization::PerSegmentGraded`
  with its per-segment grid `AxisGrading { nw, nh, width_ratio,
  height_ratio }` — the per-segment counterpart of `Discretization::Graded`.

### Changed

- Deck reader: spaces or tabs around `=` on `N`, `E`, `.default` and
  `.freq` lines (continuation lines included) are ignored, so `k = v` reads
  as `k=v`, in every mode (issue #141). A dangling `x=` is still an error.
- Deck reader: a `contact rect` / `contact decay_rect` that only touches a
  plane's boundary (or overlaps it by no more than `plane::contact_slack`)
  is dropped with a line-numbered warning instead of failing assembly on
  line 0 (issue #134).
- Deck reader: a ground-plane statement's warnings are reported in
  source-clause order (issue #136).
- Deck reader: a corner-point `G` statement's `contact rect` or `contact
  decay_rect` wholly outside the plane's footprint is no longer a plane
  assembly error on line 0 ("contact region … lies outside the plane
  footprint"); it refines nothing, so it is now dropped with the
  line-numbered warning above, consistent with the holes (issue #105). No
  deck that parsed before changes meaning. A rectangle that only touches
  the footprint is dropped with a warning too (issue #134); the `.contact`
  extension directive keeps the existing error.
- Deck reader: `file=NONE` on a corner-point `G` ground-plane statement is
  now **accepted** as the no-op it is, and a *named* file is rejected for the
  right reason (issue #122, found while implementing #113). The public
  nonuniform-plane description defines `file=` as an **input** — the file
  holding the plane's discretization hierarchy — with `NONE` meaning there is
  no such file, so the hierarchy is a single root cell discretized at run
  time from the statement's own clauses. That is the only case this reader
  ever has, so the token drops nothing; and it is the spelling the public
  description's own equivalence uses (`seg1=10 seg2=12` "could be replaced
  with `file=NONE contact initial_grid (10,12)`"), which until now this
  reader rejected on the `file=NONE` token even though it reads every other
  part of that line (issue #113). Matched without regard to case, like every
  other token this reader interprets. A named hierarchy file is still a
  line-numbered error — silently ignoring one would mesh the plane at a
  resolution the deck never asked for — but the message no longer calls
  `file=` "an output option this engine does not have": it names the file,
  says it is an input this reader does not read, and points at the
  in-statement alternatives (`seg1`/`seg2` or `contact initial_grid (n1,
  n2)`, plus the `contact` refinement clauses). No deck that parsed before
  changes meaning. See `docs/fasthenry-compat.md`'s two `file=` rows.
- Deck reader: a `contact` clause's `x…`/`y…` pair of lengths on a
  corner-point `G` ground-plane statement is now read in the **plane's own**
  coordinate system — `x…` along the `p1 → p2` edge, `y…` along `p2 → p3` —
  instead of along global x and y (issue #118). That is the frame the public
  memo (*Nonuniformly Discretized Reference Planes in FastHenry 3.0*, M.
  Kamon, 10 October 1996) defines for a corner-point plane, and the frame it
  states `contact point`'s cell sizes in by name ("0.1 is the width of the
  cell in the plane coordinate system's x-direction"); `contact line`,
  `contact rect`, `contact decay_rect`, `contact equiv_rect` and `contact
  connection` are all defined in that memo as repeated `point` calls or as an
  `equiv_rect` + `decay_rect` pair over the same widths, so the whole family
  inherits it. It is also the convention `seg1`/`seg2` and `contact
  initial_grid` (issue #113) already followed, so the plane model no longer
  uses two different frames for its two kinds of per-axis value. Affects
  `contact point`/`line`'s `xcell`/`ycell`, `contact decay_rect`'s
  `xwidth`/`ywidth`, `xcell`/`ycell` and `xmaxcell`/`ymaxcell`, and `contact
  equiv_rect`/`connection`'s `xwidth`/`ywidth`; the matching per-axis error
  messages now name the axis the deck wrote (its `xmaxcell` is what `seg1`
  counts) rather than the global axis it lands on. **Mesh change**, but only
  for a plane whose `p1 → p2` edge runs along global **y** *and* an
  anisotropic request: such a deck's pair was silently transposed before, and
  now meshes as written. For the plane every public example writes — first
  edge along global x — the mapping is the identity and nothing moves. Only
  *lengths* turn with the plane: every coordinate in the statement's body
  stays global (the memo's own examples place contacts by absolute position,
  not relative to `p1`). `contact trace` is unaffected — it carries no
  `x…`/`y…` pair and its expansion is symmetric in the two axes — and so is
  this reader's corner-spelled `contact rect` (issue #95). The evidence, the
  relative-versus-absolute question, and what would reopen either are in
  `docs/fasthenry-compat.md` § *Decision: a `contact` clause's `x…`/`y…` pair
  is in plane coordinates*.
- Deck reader: a `contact point` / `contact line` whose requested cell is
  already met by the plane's background cell on one axis now leaves that
  axis's mesh **exactly** as the plane meshes it (issue #116, follow-up to
  #100). The met axis was clamped to the background cell, as documented, but
  the region was still bounded to half a background cell either side of the
  request's own coordinate — so unless that coordinate happened to land on a
  background grid line, the already-satisfied axis still gained a band plus
  graded gap cells and every one of its edges moved. On the 10 × 6 mm,
  2 mm-cell plane of the reader's own tests, `contact point (5.3, 3, 0, 4,
  0.5)` cut an x band at 4.3 … 6.3 mm for a request x had already met; the
  clamped axis now spans the whole plane at the plane's own cell count, which
  reproduces the background edges 0/2/4/6/8/10 mm to the last bit. A point or
  line met on **both** axes still adds no region at all, and a genuinely
  refined axis is unchanged. **Mesh change**: a deck with an unaligned,
  one-axis-met `contact point` or `contact line` and no other refinement on
  that axis now has fewer cells there (its background count), so its
  impedance moves by the usual mesh-refinement amount. A deck that *also*
  refines that axis more finely elsewhere — a `contact connection` or
  `decay_rect` on the same plane, say — is unaffected as well (issue #124,
  now resolved): `GroundPlane::mesh` drops a band that spans the whole axis
  at the plane's own cell count before it merges bands, because such a band
  *is* the background mesh there — its cells are the background cells, and
  the only edges it pins are the footprint's own. Until it did, the
  whole-plane clamped band overlapped every other band on its axis and the
  existing "merged bands keep their finest cell" rule widened that finest
  cell across the whole axis: on the same test plane, `contact point (5.3,
  3, 0, 4, 0.5)` beside a 0.1 mm region near (1, 1) mm cost **100** x cells
  against the **11** that region costs on its own. A `ContactRegion` cut at
  the background cell over only *part* of an axis is not a no-op and is kept
  — it still cuts its own cells and still pins its `lo`/`hi` as cell edges.
- Deck reader: `contact circle` is now rejected with its own error, and is
  recorded as a shape the format does not have rather than as deferred work
  (issue #109, split out of #100). Looking for the argument list that was
  "still to be pinned" found that there is none: the public description of
  the `contact` family names the simple refinement utilities `point`,
  `line`, `rect` and `decay_rect`, the contact-*area* utility `equiv_rect`,
  the grouped `connection` and `trace` built on them, and the
  `initial_grid`/`initial_mesh_grid` pair — no disc among them, and
  `circle` is a **hole** shape. The error now says which family the name belongs to (so
  a deck that meant `hole circle (x, y, z, r)` is told exactly that) and
  names what a disc would map to anyway: `contact decay_rect` over its
  bounding square. That is not an approximation but the disc's cost on a
  tensor-product mesh — a refined band on one axis spans the plane on the
  other, so any refinement covering a disc refines its bounding square,
  `4/π` of the disc's area. The reasoning and what would reopen it are in
  `docs/fasthenry-compat.md`.
- Library: a refined band that is widened to absorb a sliver at the plane's
  edge, or merged with an overlapping or adjacent region, is now cut into
  the fewest cells **no larger** than its fine cell (`ceil`) rather than
  the nearest count (issue #100). Rounding could leave cells up to twice
  the requested fine cell, contradicting the documented "keeping the finest
  cell"; a band that divides exactly is unchanged, as is the graded
  validation fixture (580 bars). A graded plane whose region straddled
  that rounding now gains a cell or two on that axis.
- Deck reader / library (decision, no new API): `hole user1 (…)` …
  `hole user7 (…)` are rejected **permanently**, and no predicate or
  callback variant is added to `fasterhenry::plane::Hole` (issue #99,
  follow-up to #80). A user-defined hole is a generator compiled into the
  tool itself, so the deck carries a number and a value list whose meaning
  is stated nowhere in it — unlike the shapes #109/#110 track, it is not a
  shape awaiting a plane-model change but one no reader can recover, so the
  rejection is final and the line-numbered error now says so instead of
  pointing at a tracking issue that could never close. The error names both
  alternatives: the declarative `hole rect`/`point`/`circle` clauses, and —
  for a shape none of those describe — building the plane through the
  library. The library escape hatch needs no new API, because
  `GroundPlane::mesh` does not depend on `GroundPlane::holes`: mesh the
  plane, apply any rule to the cell centres it reports, and cut each
  selected cell with a `Hole::Point` at its own centre (a centre lies
  strictly inside its own cell, so each point removes exactly that cell —
  the composition is exact, and holds on a graded mesh). That is now a
  doctest on `Hole` and two unit tests in `fasterhenry/src/plane.rs`. A
  `Hole::Predicate` variant was considered and declined: the form that could
  carry a user hole's parameters (a boxed `Fn`) would cost `Hole` its `Copy`
  and `PartialEq` derives for a capability the composition above already
  provides exactly, and it would be public API with no caller. `Hole` stays
  `#[non_exhaustive]`, so a measured need — the composition is `O(cells ×
  holes)` — can still add it later. Reasoning recorded in
  `docs/fasthenry-compat.md` § "Decision: `hole user1`…`user7` are rejected
  permanently".
- Deck reader: `rho=` is no longer rejected with a "use sigma = 1/rho" hint
  on `.default`, `E` and extension-form `G` lines.
- No behavior change to existing valid decks from the #72 audit: the
  `.title` directive, mandatory `.units`, and `.end` semantics (required,
  rejects trailing content) are unchanged — the audit confirmed each as an
  intentional, documented difference from the public format rather than a
  bug, and recorded the reasoning in `docs/fasthenry-compat.md` (issue #72).
- Library (breaking): `ContactRegion::ratio` is now `[f64; 2]` — the
  outward decay ratio per axis — where it was a single `f64` applying to
  both (issue #80). `ContactRegion::new` and `ContactRegion::centred` still
  take one `f64` and set both axes alike, so every call through them is
  unchanged; only code reading the field, or building a `ContactRegion` as a
  struct literal, needs updating. `ContactRegion::graded_per_axis` is the
  new constructor that sets the two apart.
- Library (breaking): `Discretization` gained the `PerSegmentGraded`
  variant (issue #71). Code that matches `Discretization` exhaustively
  without a `_` arm no longer compiles, so the next release must be a minor
  bump (0.2.0).
- Library (breaking): `Discretization`, `SolveError`, `MeshError`,
  `PfftError`, `KernelError`, `PlaneError` and `inductance::Method` are now
  `#[non_exhaustive]` (issue #87). A `match` on any of them outside this crate
  now needs a `_` arm. After this change, adding a discretization mode, a
  failure mode or an evaluation method is no longer a breaking change, which
  the 0.2.0 deck-compatibility work (#76) is expected to do. Every public
  error enum is now non-exhaustive, like `SegmentError`, `GeometryError`,
  `DiscretizeError` and `CouplingError` already were. The attribute is on the
  enums, not the variants, so downstream code can still construct every
  variant directly (for example `Discretization::PerSegment(…)`). `Solver`,
  `SolverChoice`, `GridSpacing` and `inductance::Execution` stay exhaustive
  on purpose. They are closed choices, and when a new solve path arrives the
  code that dispatches on it should fail to compile rather than fall through
  a wildcard.

## [0.1.1] - 2026-09-27

### Added

- Tag-triggered release workflow (`.github/workflows/release.yml`):
  pushing `vX.Y.Z` publishes both crates to crates.io via Trusted
  Publishing (GitHub OIDC, no stored registry token), after checking the
  tag matches the workspace version and is on `main`.
- Dense/iterative size threshold, and the scaling measurement behind it
  (issue #44, phase 3 of #24). `fasterhenry::DENSE_PATH_MAX_FILAMENTS`
  (10 000) is the filament count at or below which the new
  `SolverChoice::Auto` — the default for `fasterhenry_cli::run_reporting`
  and the CLI's new `--solver auto` — keeps the dense `MeshSystem`;
  above it it selects the matrix-free `IterativeSystem`.
  `SolverChoice::Dense` / `SolverChoice::Iterative` (`--solver dense` /
  `--solver iterative`) force either path at any size, `Solver` names the
  resolved path, and `Discretization::filament_count` sizes a problem
  without assembling it so a front end can choose before paying for
  assembly. A deck that truncates coupling (`.couples`) stays dense at any
  size; `--solver iterative` on one is an error rather than a silently
  different approximation, and the CLI reports on stderr whenever a run
  leaves the dense path. The threshold sits deliberately above the
  measured ~3 000-filament wall-clock crossover — where the dense path
  stops being tractable, not where it stops being fastest — because it is
  exact where the pFFT far field approximates. New criterion bench
  `fasterhenry/benches/scaling.rs` measures both paths end to end (960 to
  99 224 filaments, wall time plus model and peak-RSS memory) and
  `docs/benchmarks.md` records the numbers, hardware and date; a budgeted
  regression guard for the matrix-free headline joins the dense one in
  `fasterhenry/tests/perf_smoke.rs`. Determinism of the iterative path is
  asserted directly: repeated solves are bit-identical and `Z(ω)`,
  GMRES iteration counts and residuals are unchanged across rayon thread
  counts.

### Changed

- Above `fasterhenry::DENSE_PATH_MAX_FILAMENTS` (10 000) filaments,
  `fasterhenry run` and `fasterhenry_cli::run_reporting` now default to
  the pFFT + GMRES path (`--solver auto`) instead of the dense one (issue
  #44). Results agree with the dense path within the documented tolerance
  (better than `1e-4` relative on `Z`), the CLI notes the choice on
  stderr, `--solver dense` restores the previous behaviour, and decks with
  `.couples` stay dense. At or below the threshold nothing changes.
  `fasterhenry_cli::cli::Command::Run` gained a `solver` field, which is
  source-breaking for code that constructs that variant or matches it
  without `..`; the `cli` module is the binary's argument definition, not
  a supported library API.

## [0.1.0] - 2026-09-25

First release to crates.io.

### Added

- Graded contact regions in the ground-plane mesh (issue #36):
  `fasterhenry::plane::ContactRegion` refines a plane locally under a via
  landing — inside the rectangle the mesh is uniform at a declared fine
  cell, outside it each cell grows by a geometric `ratio` until it reaches
  the plane's background cell. `GroundPlane::mesh` returns the resulting
  `PlaneMesh` (per-axis cell edges) instead of a single uniform
  `(dx, dy)`; grading is applied per axis so the mesh stays a conforming
  tensor-product grid with no hanging nodes, regions compose with `Hole`s
  and with each other (overlapping regions merge at the finer cell and
  gentler ratio), and a region straddling the footprint edge is clipped
  while keeping its resolution. A plane declared without regions meshes
  bit-identically to before. Decks get a matching `.contact G<name> x1 y1
  x2 y2 [nx=] [ny=] [ratio=]` directive, following `.hole`'s
  plane-name lookup. Measured on a contact-dominated fixture: the graded
  mesh matches a fully uniform 25 µm plane to 0.10 % on L and 0.80 % on R
  using 580 filaments instead of 2 992 (5.2×), and its landing-separation
  differential matches an independent PyPEEC voxel reference to 0.12 %
  (`fasterhenry/tests/plane_validation.rs`, `docs/validation.md`).
- Iterative port-impedance solve on the pFFT operator (issue #43, phase 2
  of #24): `fasterhenry::IterativeSystem` computes the same `Z(ω)` as
  `MeshSystem` without forming `L` or the dense internal-loop block
  `Z_ee`. The action of `Z_ee = M_e (R + jωL) M_eᵀ` is two sparse
  loop-basis products around two real `PfftOperator` products, and
  `Z_ee⁻¹·Z_ep` is solved by a hand-rolled restarted, right-preconditioned
  GMRES (`fasterhenry::gmres`, real or complex arithmetic, Givens-rotation
  least squares) with a Jacobi preconditioner — one solve per port. DC is
  solved in real arithmetic without the operator, so its imaginary part is
  exactly zero. `IterativeParams` bundles the pFFT and GMRES parameters;
  a GMRES solve that misses its tolerance is reported as
  `SolveError::NotConverged`, and an operator that cannot be built as
  `SolveError::Pfft`. Matches the dense path to better than `1e-7`
  relative on the spiral and coupled-structure fixtures (tested at `1e-4`,
  including a floating ring and DC). The dense `MeshSystem` stays the
  default; the size threshold between the two is issue #44.
- Matrix-free partial-inductance operator by the precorrected FFT
  (issue #42, phase 1 of #24): `fasterhenry::pfft::PfftOperator` evaluates
  `L·x` without assembling `L` — filaments projected onto a uniform grid by
  Lagrange interpolation, the far field convolved with `1/r` by zero-padded
  (and pruned) 3-D FFT, and near pairs precorrected to the exact kernel
  value. Grid spacing, near-field radius and interpolation order are named
  `PfftParams`, with the accuracy-versus-cost trade-off measured and
  documented; the defaults agree with the dense product to about `2e-7`
  relative, and `fasterhenry/tests/pfft.rs` checks `1e-6` on random sets,
  the spiral and coupled-structure fixtures, and flat, line-like and
  single-cell geometries. Memory and time per product grow linearly with
  the filament count (criterion bench `fasterhenry/benches/pfft.rs`,
  1 000–10 000 filaments). The solver is unchanged; iterative solution on
  the operator is issue #43. New dependency: `rustfft` (pure Rust, MIT OR
  Apache-2.0).
- Width-graded skin-effect validation against an independent 2-D reference
  (issue #32): `tools/cross_section_reference.py` solves the finite-width
  cross-section of an isolated rectangular trace (Richardson-extrapolated,
  second-order convergence enforced) and
  `fasterhenry/tests/width_graded_skin_validation.rs` compares uniform,
  width-graded and skin-depth-adaptive grids against it at `t/δ = 0.3 … 10`,
  asserting a refinement-based accuracy bound for `R'` and the internal
  inductance change `ΔL'`. It shows width grading is the accurate grid; a
  width-uniform 36 × 10 grid reads `R'` 28 % low at `t/δ = 10`. Results and
  the supported regime are recorded in `docs/validation.md`; CI regenerates
  the reference and requires every gate to report `PASSED`.
- Committed criterion bench suite: `fasterhenry/benches/assembly.rs` and
  `fasterhenry/benches/solve.rs` measure `MeshSystem::assemble` and
  `MeshSystem::sweep` throughput against a fixed filament-count sweep
  (48–19 600, matching the scale of `docs/benchmarks.md`'s head-to-head
  table), alongside the existing kernel-level `benches/kernels.rs`. A new
  `workflow_dispatch` CI job (`.github/workflows/bench.yml`) runs the
  suite on a pinned `ubuntu-latest` runner and regenerates a dated results
  table in `docs/benchmarks.md` via `tools/bench_table.py`, parsed from
  criterion's own JSON output rather than hand-typed.
- Coupling truncation (`.couples`): tag segments with `group=<name>` and
  declare which groups are magnetically coupled; the mutual inductance of
  every other pair is truncated to zero and — the point of the knob — never
  evaluated. `fasterhenry::coupling` is the library API (`Coupling`,
  `MeshSystem::assemble_with_coupling`, `PairMask`), and documents when the
  approximation is safe: closed loops more than about ten of their own
  extents apart cost under a part in a thousand, since the leading term falls
  as `(a/d)³`. Truncated groups closer than one extent are reported on
  stderr. A deck with no `.couples` line, and `.couples all`, keep today's
  all-pairs assembly bit for bit.
- Ground planes (`G` directive with `.hole`): thick rectangular sheets
  discretized into a cell-centre bar mesh on the same kernels/mesh/solve
  machinery; segment and port endpoints landing in a plane's footprint
  snap to the nearest live cell node (assembly order-independent,
  `.equiv` preserved). Deck syntax, library `fasterhenry::plane` API,
  and validation: trace-over-plane DC/grid checks plus the PyPEEC
  slot-differential gate (4.97 % measured, 5.5 % bound; CI regenerates
  the references and asserts the test did not skip).
- Surface-graded filament grids and skin-depth-sized subdivision for the Rust
  API. The existing uniform grid and `.inp` `nwinc`/`nhinc` path are unchanged.
  A wide-trace validation reaches 1.2 % resistance error against the
  one-dimensional skin-effect asymptote; see `docs/validation.md`.

- Output formats for tool interop: `--zc-mat` writes a binary MAT v4
  `Zc.mat`-format file (`Zc_1 … Zc_K` complex matrices + `freqs`; exact
  round-trip asserted against `scipy.io.loadmat` in CI), and `--spice
  [--spice-freq HZ]` writes a SPICE subcircuit stamping `Z = R + jωL`
  (coupled inductors for L, H sources for R; ngspice-verified to
  reproduce Z to six digits). See `docs/output-formats.md`. The previous
  plain-text `--zc-mat` experiment is superseded (pre-release CLI).
- Validation harness (M0's "validated" gate): the 2-turn square spiral
  fixture checked against PyPEEC 5.8.0 (2 % tolerance; `tools/pypeec_reference.py`
  regenerates the reference, CI regenerates it on the Linux legs and
  asserts the test did not skip), Greenhouse segment summation (5 %), and
  the analytic DC resistance; Mohan modified-Wheeler reported for the
  record. Measured: 0.47 % vs PyPEEC, 0.12 % vs Greenhouse. See
  `docs/validation.md`.
- `fasterhenry run <deck.inp | problem.json> [--freq FMIN FMAX NDEC]
  [--json OUT] [--zc-mat OUT]`: the CLI's first real subcommand. Reads an
  M0-subset FastHenry `.inp` deck (public format; clean-room parser with
  line-numbered errors) or a JSON problem document, runs the sweep, and
  writes the JSON result (with provenance) and optionally a plain-text
  impedance matrix. `--version` prints the crate version plus a
  build-time `git describe`.
- Round-trip guarantee: a self-authored two-turn spiral as an `.inp` deck
  and as the equivalent JSON document produce bit-identical sweeps
  (`fasterhenry-cli` integration test).
- `--freq` overrides the deck's sweep with the same decade sampling `.freq`
  uses (`fmin == fmax` runs one frequency; `0` runs the DC solve).

- Cargo workspace with the `fasterhenry` library crate and the
  `fasterhenry-cli` crate, which installs the `fasterhenry` binary.
- Packaging metadata for both crates (`readme`, `documentation`, `keywords`,
  `categories`), per-crate READMEs, and an `include` allowlist so a published
  package carries only sources, the manifest, README, LICENSE, and this
  changelog.
- CI packaging job: `cargo publish --dry-run` for the library and a
  package-contents check for both crates.
- Non-transitive `.couples` declarations are rejected (issue #46): when two
  declared groups are reachable only through a third — `.couples a b` plus
  `.couples b c` with no `a`–`c` — `Coupling::resolve` returns the new
  `CouplingError::NonTransitiveCoupling`, naming the undeclared pair. Such a
  relation can make the truncated `L` indefinite, and every previously
  accepted deck of this shape was silently solving an unphysical system;
  there is no regime where the shape is intended (declare the whole clique,
  or use the `.couples a b c` shorthand). Decks with no `.couples` line and
  `.couples all` are unaffected, as are fully declared cliques and isolated
  pairs.

### Changed

- `fasterhenry::plane::GroundPlane` gained a `contacts: Vec<ContactRegion>`
  field (issue #36). It is `Default`-able and empty means "uniform, exactly
  as before", but a struct-literal construction that lists every field now
  has to name it (`contacts: Vec::new()`) — the only source-breaking part
  of that issue.
- `PfftOperator::new` set-up: the near-field precorrection's local box
  potential (`near::grid_interactions`, issue #51's profiling found it about
  7-8× the cost of the exact kernel evaluation it is paired with, and about
  75% of total set-up at 10 000 filaments) now accumulates with a SIMD
  multiply-add instead of a scalar loop, about 10-15% off total set-up time
  at that size. A reusable phase-by-phase profiling test
  (`cargo test --release -p fasterhenry --lib pfft::near::profile --
  --ignored --nocapture`) and the profiling results, including the levers
  considered and set aside, are documented in the `pfft` module docs
  ("Set-up cost"). Accuracy and the `O(n)` scaling from issue #42 are
  unchanged.
- `fasterhenry-cli` now takes the library from `[workspace.dependencies]` with
  both a `path` and a `version`, so the CLI crate is publishable.

[Unreleased]: https://github.com/2AMLogic/fasterhenry/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/2AMLogic/fasterhenry/releases/tag/v0.1.1
[0.1.0]: https://github.com/2AMLogic/fasterhenry/releases/tag/v0.1.0
