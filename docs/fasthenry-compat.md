# FastHenry `.inp` format compatibility

`fasterhenry`'s deck reader (`fasterhenry-cli/src/inp.rs`) accepts a subset of
the public FastHenry `.inp` input-format description. This table is the
line-by-line audit against that public description (issue #72, part of the
0.2.0 deck-compatibility epic, #76): every directive and framing rule the
format documents, whether this reader supports it, and — for every
difference — why.

Per this repository's clean-room rule (`CONTRIBUTING.md`), this audit and the
reader itself are written from the public format description and
self-authored example decks only; no FastHenry/FastCap source was consulted.

Status key:

- **Supported** — matches the public description.
- **Supported, extended** — matches, plus an additive feature the public
  format does not document (never a name the format already uses for
  something else).
- **Supported, differs** — intentionally different behavior, with the reason
  below.
- **Deferred** — tracked by a sibling issue in the same epic.
- **Not supported** — rejected with a line-numbered error; out of scope here.
- **Not supported, permanent** — rejected with a line-numbered error, and
  the rejection is a recorded decision rather than pending work; the
  reasoning is written out below the table it appears in.

## Deck framing

| Rule | Status | Notes |
|---|---|---|
| First line is a title, always ignored | Supported, differs (opt-in) | By default this reader has no implicit first line: line 1 is parsed like any other, so prose there is rejected, and `.title <text>` is an explicit, additive directive instead — kept as the default deliberately: this parser's whole design rejects anything it cannot place (unknown fields, unknown directives, a missing `.units`) rather than silently reinterpreting a line, and unconditionally discarding line 1 would silently swallow the unit directive of a deck that opens with `.units m`. For genuine third-party decks, `fasterhenry run --fasthenry-compat` (library: `inp::ParseOptions { fasthenry_compat: true }` with `inp::parse_with_options`, issue #83) reads the file's *physical* first line as an always-ignored title, whatever it holds — prose, nothing, a `*` comment, or directive-looking text — and it becomes the deck's title (none when blank). It is the physical first line rather than the first non-blank one, so a blank line 1 cannot cost the deck its first real directive; error line numbers still count it, and a `+` line straight after it is an error (a title is not continued). A `.title` directive later in the deck is still honored in this mode and replaces the line-1 title: it is additive and unambiguous, and disabling it would turn a line this reader otherwise accepts into an error for no safety gain. |
| `*` starts a comment line | Supported | `inp.rs` drops any line whose trimmed text starts with `*` before it reaches directive parsing. |
| `+` starts a continuation line | Supported | The rest of a `+` line's tokens append to the previous line; a deck that opens with a `+` line is an error (there is no previous line to continue). |
| Blank lines | Supported | Dropped like comments, anywhere in the file, including trailing lines after `.end`. |
| Whitespace around `=` (`k = v`, `k= v`, `k =v`, tabs) | Supported | Issue #141. In every mode, on `N`, `E`, `.default` and `.freq` lines — continuations included — a `key`, `=`, `value` split by spaces or tabs is rejoined into one `key=value` before the line is read, so every spacing parses exactly like `k=v`; `G` lines already allowed it in their own scanner. A dangling `x=` with no value (at the end of the statement, or followed by another assignment) is still a line-numbered error naming the field. |
| Directive keywords case-insensitive (`.UNITS` = `.units`) | Supported | Matched via `to_ascii_lowercase()`. |
| Node/element names case-sensitive | Supported | `N1` and `n1` are different nodes; every reference (`.external`, `E` endpoints, `.equiv`) is matched by exact string equality. |
| `.end` required | Supported, differs | A deck without `.end` is rejected. FastHenry decks conventionally end with `.end`, but this reader treats its absence as an error rather than an implicit end-of-file — consistent with the "no default swallows a mistake" design already applied to `.units`. |
| Content after `.end` | Supported, differs | Comments and blank lines after `.end` are fine (dropped before the `.end` state is even consulted, same as elsewhere in the file); another directive/node/segment line after `.end` is an error. Multi-deck-in-one-file constructs some formats allow after a terminator are out of scope: this reader parses exactly one deck per input. |

## `.units`

| Field | Status | Notes |
|---|---|---|
| `.units <unit>` | Supported, differs (mandatory) | The engine takes the presence of `.units` as mandatory rather than defaulting silently — a deliberate safety choice, so a missing unit line can never scale every length (and therefore every impedance) by a factor of 10 or 100 without a diagnostic. A deck without `.units` is rejected with a line-numbered error naming the missing directive rather than assuming a default unit. Under `--fasthenry-compat` (issue #144) a missing `.units` reads as metres, as FastHenry does, with a line-numbered warning on the first line that needs a unit (`no .units directive before this line; reading lengths in metres, …`). |
| Several `.units` lines | Supported, differs (opt-in) | The public guide honours each `.units` from its own line onward. By default a second `.units` is a line-numbered error (`duplicate .units directive`). Under `--fasthenry-compat` (issue #144) each applies forward only — to lengths and explicit conductivities (`sigma=`/`rho=`) on later lines — never retroactively: every value already read, `.default` fields included, is stored in metres and S/m at the unit in force on its own line, so a `.default sigma=5.8e4` under `.units mm` stays 5.8e7 S/m after a later `.units m`. |
| Full documented unit list: `km`, `m`, `cm`, `mm`, `um`, `in`, `mils` | Supported | Case-insensitive (issue #70); `mil` is also accepted as a synonym for `mils`. |
| Long spellings FastHenry reads correctly: `meter(s)`, `metre(s)`, `kilometer(s)`, `kilometre(s)`, `inch`, `inches` | Supported, differs (opt-in) | Issue #144. FastHenry matches `.units` by prefix, so it reads these at the right scale. Rejected by default (`unknown length unit`); under `--fasthenry-compat` accepted at the documented unit's factor with a line-numbered warning naming the documented spelling. Case is ignored. |
| Spellings FastHenry misreads: `millimeter…`/`millimetre…`/`milli…`, `micron…`/`micrometer…`/`micrometre…` | Rejected, differs (deliberate) | Issue #144. FastHenry's prefix match reads the first family as **mils** (2.54e-5 m) and the second as **metres**, silently solving at the wrong scale. This reader rejects them in both modes, with a line-numbered error that, under `--fasthenry-compat`, names FastHenry's misreading and the documented spelling (`mm`, `um`): matching FastHenry here would reproduce a wrong answer. Other spellings FastHenry rejects (`centimeter`, `nm`, `ft`, bare `c`/`u`/`i`) are the ordinary unknown-unit error. |

## `.default`

| Field | Status | Notes |
|---|---|---|
| `.default x= y= z= w= h= nwinc= nhinc= rw= rh= wx= wy= wz= sigma=` | Supported | Applies to every `N`/`E` line parsed after it; not retroactive. Requires `.units` first (lengths need a scale factor before they can be stored); under `--fasthenry-compat` a deck with none is in metres, with a warning (see `.units`). |
| `.default rho=` | Supported | Issue #70. Per deck unit, the exact reciprocal of `sigma=`; must be positive. Naming both `sigma=` and `rho=` on one line is a line-numbered error; a later per-line value in either form overrides a `.default` in either form. |

## Nodes (`N`)

| Field | Status | Notes |
|---|---|---|
| `N<name> x= y= z=` | Supported | A coordinate missing from both the line and `.default` is a line-numbered error naming which coordinate and which node. |

## Segments (`E`)

| Field | Status | Notes |
|---|---|---|
| `E<name> N<a> N<b> w= h= sigma=\|rho=` | Supported | `w`/`h` are stored as magnitudes (`abs()`); a missing `w`, `h`, or conductivity (`sigma`/`rho`, line or `.default`) is a line-numbered error naming the segment and the field — except a missing conductivity under `--fasthenry-compat` (next row). |
| `E … sigma` omitted (no `sigma=`/`rho=` on the line or in `.default`) | Supported, differs (opt-in) | Issue #142. FastHenry gives such a conductor the conductivity of copper. Under `--fasthenry-compat` this reader does the same: 5.8e7 S/m as a **physical** value, independent of `.units` (σ in deck units is 5.8e7 × the unit's length in metres — unlike an explicit `sigma=`, which is per deck unit, so under `.units mm` the default matches `sigma=5.8e4`, not `sigma=5.8e7`), with a line-numbered warning on the statement (`segment 'E1' has no conductivity; using copper (5.8e7 S/m, FastHenry default)`). A conductivity on the line or in `.default` still takes precedence and raises no warning. Operator decision: defaults under compat, strict otherwise — without the flag a missing conductivity stays a line-numbered error, in keeping with this reader's "no silent defaults" design (as for `.units`), because a conductor whose material was simply forgotten would otherwise solve as copper without a word; third-party decks that rely on the documented default read under the flag, and even there the default is reported, not silent. |
| `nwinc=`, `nhinc=` | Supported | Per-segment filament subdivision counts. |
| `rho=` | Supported | Issue #70; same rules as `.default rho=`. |
| `rw=`, `rh=` | Supported, differs | Issue #71. The ratio of adjacent filament extents across the width / height, each axis independently, coarsening from both surfaces inward (`nwinc=5 rw=2` cuts the width 1:2:4:2:1). A ratio must be a number ≥ 1 (below 1 is rejected, not guessed at); an omitted ratio keeps the uniform grid, so decks without `rw`/`rh` solve exactly as before. See `inp.rs`'s `# Semantics` section. |
| `wx=`, `wy=`, `wz=` | Supported | Issue #71. A dimensionless vector along the cross-section's width (only its component perpendicular to the segment counts); a component given nowhere is 0 once any is given. A zero vector, or one parallel to the segment, is a line-numbered error. |
| `group=<name>` | Supported, extended | This reader's own addition, feeding `.couples` (below) — not a documented FastHenry field. Case-sensitive, like node names. |

## `.external`

| Field | Status | Notes |
|---|---|---|
| `.external N<+> N<->` | Supported | Current flows in at the first node, out at the second. |
| Optional trailing `[name]` | Supported, extended | Labels the port in output; the public format has no third field. Defaults to `<+>/<->` when omitted. |

## `.freq`

| Field | Status | Notes |
|---|---|---|
| `.freq fmin= fmax= ndec=` | Supported | Log-spaced decade sweep, `ndec` points per decade; see `frequency_sweep` and the module's `# Semantics` section for the exact formula. |
| `fmin = fmax` | Supported | The single-frequency case, at any `ndec`. |
| `fmin = 0` | Supported, differs | Allowed only when `fmin = fmax` (the DC solve) — `log10(0)` is undefined, so a genuine log sweep starting at zero has no well-defined first point. Rejected with a line-numbered error rather than silently starting somewhere else. |

## `.equiv`

| Field | Status | Notes |
|---|---|---|
| `.equiv N<a> N<b>` | Supported | `b` becomes an alias of `a`. |
| `.equiv N<a> N<b> N<c> …` (more than two nodes) | Supported | Every later name aliases to the first (`.equiv a b c` is `.equiv a b` followed by `.equiv a c`). Landed with the ground-plane work (issue #69); this audit adds `equiv_joins_more_than_two_nodes` and `equiv_chain_rejects_a_node_repeated_with_itself` to `inp.rs`'s test module to pin the behavior. Naming a node together with itself is a line-numbered error. |
| Resolution order (a reference before vs. after the directive) | Supported | Every reference resolves through the alias chain regardless of where in the deck it appears; the deck is one circuit, not a timeline. |
| Joining an in-plane node (`G` statement `N<name> (x, y, z)`) | Supported, differs | If any node in the joined set is an in-plane node, the whole set lands on that plane's nearest live cell-centre node, whichever side of the directive named it — so wiring a via into a plane does not depend on argument order. Joining in-plane nodes of two *different* planes is rejected (connect planes with a segment) rather than silently attaching to one. |

## Ground planes (`G`, `.hole`, `.contact`)

Supported since issue #69. `inp.rs`'s module documentation ("Ground planes:
two `G` grammars") is the authoritative description; this table only
records how each documented field maps.

| Field | Status | Notes |
|---|---|---|
| `G<name> x1= y1= z1= x2= y2= z2= x3= y3= z3=` (three corner points) | Supported, differs | The plane must be axis-aligned and parallel to the xy plane (`z1 = z2 = z3`, each edge along x or y): this engine's plane model is an axis-aligned rectangle. A tilted or rotated plane is rejected by name rather than squared off. The corner points give the plane's mid-thickness surface, as a segment's nodes give its axis. The three points also fix the **plane's own coordinate system** — `p1` the origin, `p1 → p2` its x-direction, `p2 → p3` its y-direction — which is the frame every `x…`/`y…` pair of *lengths* in the clauses below is stated in; see "Decision: a `contact` clause's `x…`/`y…` pair is in plane coordinates" below (issue #118). |
| `thick=` | Supported | Plane thickness. |
| `seg1=`, `seg2=` | Supported, differs | Become the background cell counts of this engine's own cell-centre PEEC mesh, not FastHenry's panel mesh: equal counts mean equal resolution, not an identical node set. |
| `sigma=` | Supported, differs (opt-in) | Per deck unit, falling back to the `.default` conductivity (`.default sigma=` or `.default rho=`). A plane with neither is a line-numbered error by default; under `--fasthenry-compat` it is copper, 5.8e7 S/m physical whatever the `.units`, with a line-numbered warning on the statement (`ground plane 'G1' has no conductivity; using copper (5.8e7 S/m, FastHenry default)`), exactly as for a segment and for the same reason — operator decision: defaults under compat, strict otherwise (issue #142; see `E … sigma` above). The extension `G` form follows the same rule. |
| `nhinc=` | Supported | Filaments through the plane's thickness. |
| `rho=` | Supported | Issue #88. Per deck unit, the exact reciprocal of `sigma=` and accepted on the corner-point statement itself (continuation lines included); naming both `sigma=` and `rho=` on one statement is a line-numbered error, as it is elsewhere. |
| `relx=`/`rely=`/`relz=` | Supported | Issue #146. The documented offset (User's Guide §1.3.9), default 0, in every mode: it is added to every in-plane coordinate on the statement — each `N<name> (x, y, z)` node reference, each `hole` shape's points or centre, and each `contact` clause's points, ends or centre (`rect`, `decay_rect`, `point`, `line`, `trace`, `equiv_rect`, `connection`) — but **not** to the corner points `x1…z3`, so the mesh is unchanged while the connections move. A `relx=0.3` deck reads exactly as the same deck with those coordinates written 0.3 larger. Only coordinates (points, centres) move; widths, cell sizes and radii do not (they are lengths — see the plane-coordinate decision below, issue #118). Position-independent: a key written after the clauses still applies to them, and a repeated key's last value wins for every point. A `relz` that lifts a clause or node out of the plane's slab is the same line-numbered error the shifted coordinate would be. |
| Empty coordinate field in a node reference, `N<name> (, y, z)` | Supported (compat only) | Issue #146. Under `--fasthenry-compat`, one empty field reads as 0 before the `relx`/`rely`/`relz` offset, with a line-numbered warning. Natively it is a line-numbered error, and two or more empty fields are an error in both modes. Clause value lists (`hole …`, `contact …`) are unaffected. |
| `rh=`, `segwid1=`/`segwid2=` | Not supported | Each rejected by name with the reason and the alternative (plane filaments are uniform; bar widths follow the cells). Nothing on a `G` statement is silently ignored. |
| `file=NONE` | Supported | Issue #122. The nonuniform-plane description defines `file=` as an *input* — the file holding the plane's discretization hierarchy — and `NONE` as "no such file": the hierarchy is a single root cell, discretized at run time from the statement's own clauses. That is the only case this reader ever has, so `file=NONE` is accepted as the no-op it is and the documented `file=NONE contact initial_grid (10,12)` spelling reads as written. Matched without regard to case, like every other token this reader interprets. |
| `file=<name>` (a named hierarchy file) | Not supported | Issue #122. Rejected by name on the statement's own line: a stored nonuniform-discretization hierarchy is an *input* this reader does not read (not, as the message used to say, an output it does not write), and silently ignoring it would mesh the plane at a resolution the deck never asked for. The error names the file, says only `file=NONE` is accepted, and points at the in-statement alternatives — `seg1`/`seg2` or `contact initial_grid (n1, n2)` plus the `contact` refinement clauses, which is what this reader discretizes from. |
| In-plane node `N<name> (x, y, z)` | Supported | An ordinary deck node belonging to the plane; a reference to it lands on the nearest live cell-centre node of that plane. |
| `hole rect (x1, y1, z1, x2, y2, z2)` | Supported | Two opposite corners, as documented, onto the plane model's rectangular hole. The redundant `z` coordinates are checked against the plane's slab, so a rectangle meant for another plane cannot land here silently. A rectangle (like a `hole point`/`circle`, `contact rect` or `contact decay_rect`) wholly outside the plane's footprint is accepted with a line-numbered warning — see the decision below (issue #105). |
| `contact rect (x, y, z, xwidth, ywidth, xcell, ycell)` | Supported | Issue #95. The **documented** spelling — the rectangle's centre, its full widths about that centre, and the largest cell wanted inside it, the same shape of argument list every other `contact` shape uses. It is exactly `contact decay_rect` without the outward limits, and is read as one with both `maxcell`s at the negative "no limit" sentinel, so the row below describes it in full: `ceil(width/cell)` fine cells per axis, the documented decay law `1/(1 − cell/width)` outside, `cell` smaller than `width`. |
| `contact rect (x1, y1, z1, x2, y2, z2)` | Supported, extended | Issue #95. Two opposite corners, as `hole rect` spells them — **not** documented FastHenry syntax but this reader's own extension, kept because decks written against earlier releases use it. Refined to 2 × 2 fine cells at ratio 2, which is the seven-value form at `xcell = xwidth/2`, `ycell = ywidth/2`: `contact rect (4, 2, 0, 6, 4, 0)` and `contact rect (5, 3, 0, 2, 2, 1, 1)` are the same region. The two spellings are told apart by **value count alone**, and any other count is a line-numbered error naming both — see the decision below. |
| `contact decay_rect (x, y, z, xwidth, ywidth, xcell, ycell, xmaxcell, ymaxcell)` | Supported, differs | Issue #80. Maps onto `fasterhenry::plane::ContactRegion`: centre and full widths give the rectangle, `ceil(width/cell)` per axis gives the fine cells, and the documented decay law `1/(1 − cell/width)` gives that axis's growth ratio. All three `x…`/`y…` pairs are in the plane's own coordinate system (`x…` along `p1 → p2`), per the decision below; the centre is a global coordinate. `cell` must be smaller than `width` (the documentation's own `r0 < 1`). The differences are both in the outward limit: this engine's grading levels off at the plane's **background cell** rather than at `maxcell`, so a positive `maxcell` **finer** than the background cell is rejected by name (raise `seg1`/`seg2` instead of shipping a quietly coarser mesh), while one at or above it never binds; a negative `maxcell` — the sentinel the documented `contact connection` shorthand expands to — asks for no limit. The resulting mesh is this engine's own graded cell-centre mesh, not FastHenry's cell subdivision, so equal cell sizes mean equal resolution, not an identical node set. |
| `hole point (x, y, z)`, `hole circle (x, y, z, r)` | Supported | Issue #98. Map onto `fasterhenry::plane::Hole::Point` and `Hole::Circle`, widening the plane's hole model beyond the axis-aligned rectangle. A point removes exactly the one cell whose own extent (edges included) contains it; a point landing exactly on a shared cell edge or corner is the documented tie and removes every cell touching it, rather than guessing a single winner. A circle removes every cell whose centre lies at or inside its radius `r` — a centre exactly on the circle counts (a closed boundary, unlike `hole rect`'s open one, since there is no prior rectangle behaviour to match). Both check their own `z` against the plane's slab like every other hole/contact clause. |
| `contact equiv_rect N<name> (x, y, z, xwidth, ywidth)` | Supported, differs | Issue #101. The named **contact area**: the rectangle (centre and full widths, as `decay_rect` spells it — and, like `decay_rect`'s, the widths are in the plane's own coordinate system, per the decision below) is tied to the one node the clause names, so a deck can `.equiv` an external node onto a landing *pad* rather than a point. It maps onto the new `fasterhenry::plane::Equipotential`: every live cell centre inside the rectangle — boundary included — shares one node, the bars between those cells are not built, and the bars crossing the patch's boundary end on that node, which sits at the mean of the cell centres it ties. That last part is the difference worth stating: the model is exact for a patch covering one cell, and for a larger patch each entering bar reaches the tie through metal the mesh still treats as ordinary plane rather than as the perfect conductor the patch is — an over-, never under-, estimate of the contact's own resistance, and one that (unlike landing on a single cell centre) does not grow as the mesh is refined. Two rectangles that tie a cell in common merge into one equipotential. The node name follows the in-plane-node rule (`N`-prefixed); a rectangle catching no live cell centre is a line-numbered error naming the plane rather than a silent landing on a neighbouring cell. |
| `contact connection N<name> (x, y, z, xwidth, ywidth, ratio)` | Supported | Issue #101. The documented shorthand: exactly a `contact equiv_rect` over the rectangle plus a `contact decay_rect` over the same rectangle with cells `xwidth/ratio`, `ywidth/ratio` and no decay limit (the negative-`maxcell` sentinel in the row above). Writing the two clauses out by hand produces an identical deck, which is what `connection_is_an_equiv_rect_plus_a_decay_rect` in `inp.rs` asserts. `ratio` must be > 1 — it divides the widths into the contact's own cells — and a ratio of 1 or less is a line-numbered error rather than a rectangle that grades nothing. The widths are in the plane's own coordinate system, as in both halves it expands to (decision below). |
| `hole user1 (…)` … `hole user7 (…)` | Not supported, permanent | Issue #99. Rejected by name on the statement's own line, and the rejection is final rather than deferred — see the decision below. The error names the alternatives: the declarative shapes above, and `fasterhenry::plane::GroundPlane::mesh` + `fasterhenry::plane::Hole::Point` for a shape none of them describe. |
| `contact point (x, y, z, xcell, ycell)`, `contact line (x0, y0, z0, x1, y1, z1, xcell, ycell)` | Supported, differs | Issue #100. Every cell holding the point, or crossed by the line, is no larger than `xcell` × `ycell` — the pair measured in the plane's own coordinate system, `xcell` along `p1 → p2` (the decision below; this is the clause the public memo states that convention for outright), while the point's or line's own coordinates are global. Both map onto one `fasterhenry::plane::ContactRegion`: the locus's bounding box padded by half a requested cell on every side, cut into the fewest cells no larger than that cell — so a point is exactly one `xcell × ycell` cell centred on it, and a zero-length line is that point. Outside it the mesh grades back at ratio 2, the reader's `contact rect` default (neither shape documents a decay). A requested cell at or above the plane's background cell is already met and is clamped there rather than coarsening the mesh; a point or line met on both axes adds nothing. Both ends are checked against the plane's slab and footprint. **The difference**: a diagonal line refines its whole padded bounding box, because this engine's mesh is a tensor product — a refined band on one axis spans the plane on the other, so any refinement covering the line (a chain of rectangles included) has the same bands. That costs about `(Lx/xcell)·(Ly/ycell)` fine cells where a mesh following the diagonal would need about `Lx/xcell + Ly/ycell`; an axis-aligned line costs nothing extra. |
| `contact trace (x0, y0, z0, x1, y1, z1, trace_width, scale_factor)` along x or y | Supported | Issue #110. Refines the plane finely across the trace's projection, not along it. Source: the public memo *Nonuniformly Discretized Reference Planes in FastHenry 3.0* (M. Kamon, 10 October 1996, section "Grouped contact utilities"), whose worked example expands an x-directed trace of width `w` and length `L` into five `contact line`s — at the trace and at `±w/2` with cells `(L, w/2)`, and at `±3w/2` with cells `(L, w)` — and which states that `scale_factor` has no effect on a trace parallel to x or y. This reader performs exactly that expansion (axes swapped for a trace along y), each line mapping as in the `contact line` row above; writing the five lines out by hand gives the same mesh, which `contact_trace_along_x_is_the_documented_five_lines` in `inp.rs` asserts. `scale_factor` is checked to be positive and otherwise unused. The trace's own ends are checked against the plane's slab and footprint on the statement's line; the side lines are clipped to the plane (and dropped where they miss it), so a trace along the plane's edge is accepted. A zero-length trace has no direction and is a line-numbered error naming `contact point`. The clause carries no `x…`/`y…` pair of its own — one `trace_width`, and a direction read from its own global ends — and its expansion is symmetric in the two axes, so the plane-coordinate decision below does not touch it: rotating the plane rotates the trace with it (issue #118). |
| `contact trace (…)` not parallel to x or y | Native: not supported, permanent. `--fasthenry-compat`: supported, approximated | Issue #110 (native), issue #157 (compat). Under compat the trace is refined as its bounding box padded by `3w/2` on every side — a `contact rect` over it with cells `(w/2)·scale_factor^min(|tan θ|, \|cot θ\|)` on both axes, θ measured in the plane's own frame so 30° and 60° mirror each other — with a line-numbered warning that this approximates FastHenry's staircase refinement (see the decision below). Natively: rejected by name on the statement's own line, as a decision rather than deferred work — see "Decision: a diagonal `contact trace` is rejected" below. The error names `contact line (x0, y0, z0, x1, y1, z1, xcell, ycell)`, which states the cell size outright. |
| `contact circle (…)` | Not supported, permanent | Issue #109. Rejected by name on the statement's own line — and, like the diagonal `contact trace` row above, this is not deferred work: the public description of the `contact` family names the simple refinement utilities `point`, `line`, `rect` and `decay_rect`, the contact-*area* utility `equiv_rect`, the grouped `connection` and `trace` built on them, and the `initial_grid`/`initial_mesh_grid` pair — no disc among them. `circle` is a **hole** shape (`hole circle (x, y, z, r)`), so there is no argument list to pin and nothing in a deck to read — see the decision below. The geometry is not what is missing: on this tensor-product mesh the whole of what a disc could mean is `contact decay_rect` over its bounding square, and the error names that. |
| `contact initial_grid (n1, n2)` | Supported | Issue #113, which pinned the convention #101 left open: `n1` cells along `p1 → p2` and `n2` along `p2 → p3` — the same pair `seg1`/`seg2` set, as the clause's own public description states ("`seg1=10 seg2=12` could be replaced with `file=NONE contact initial_grid (10,12)`"). See the decision below for the evidence, including why the description's word *rows* does not overturn it. The counts are cells, not grid lines. Because the two clauses are one statement, a plane giving both `contact initial_grid` and `seg1`/`seg2` (in either order), or two initial grids, is a line-numbered error rather than a race between them. `file=NONE`, which the public example pairs with the clause, is accepted as the no-op it is (issue #122 — see its own row above), so the documented line reads as written; the token says there is no discretization hierarchy file, which is the only case this reader has. |
| `contact initial_mesh_grid (n1, n2)` | Supported, differs | Issue #113. That same initial grid, plus the documented checkerboard: "every cell that has an even value for both of its indices where the numbering is from the top left" — so, with the cells numbered from 1 at the plane's own origin (its `p1` corner) along each axis, every cell whose two indices are **both even**. An axis of one cell has no even index and so no hole. The difference is what each hole *is*: a `Hole::Rect` over the holed cell's own rectangle rather than a `Hole::Point` at its centre. On the initial grid alone the two are the same cut; the grid is *initial*, though, and a later `contact` clause may refine that region — the documented hole is the square ("no conductor will be defined in that square region"), not whichever smaller cell a refinement leaves under the centre. Numbering from `p1` is what fixes the checkerboard's phase: for an odd count either end selects the same cells, but for an even count the two differ by one cell, and `p1` is the origin of the plane coordinate system the same description defines (and the corner its own uniform-plane figure draws at the top left). |
| `G<name> x1 y1 z1 x2 y2 z2 t [nx=] [ny=] [nhinc=] [sigma=\|rho=]`, `.hole`, `.contact` | Supported, extended | This project's own shorthand plane form and its separate refinement directives — not documented FastHenry syntax. Told apart from the corner-point form by the first token after the plane name, so a deck may mix the two; both build the same plane. |

### Decision: `contact rect` reads both spellings, told apart by value count (issue #95)

Until issue #95 this reader spelled `contact rect` as **two opposite
corners**, `(x1, y1, z1, x2, y2, z2)`, mirroring `hole rect`. The documented
clause is not that: like `contact point`, `contact line`, `contact
decay_rect` and `contact equiv_rect`, it takes a **centre**, the
rectangle's **full widths** about that centre and the largest **cell**
wanted inside it — `(x, y, z, xwidth, ywidth, xcell, ycell)`, seven values.
`hole rect` really is two corners, but it belongs to the *uniform* plane
syntax; the `contact` shapes belong to the non-uniform one, and the two
conventions do not match. So a genuine third-party deck was rejected for
its value count, and a six-value clause this reader accepted meant
something other than what that spelling means in the documentation.

**The decision: accept both, disambiguated by arity.** Seven values are the
documented centre/widths/cell form; six keep the corner meaning. Three
reasons:

1. **The counts cannot collide.** Six and seven are distinct, so no deck is
   ambiguous and no clause changes meaning silently. The corner form is
   unreachable from the documented one and vice versa, which is what makes
   one clause name with two grammars safe here rather than a guess.
2. **Nothing that parsed before changes meaning.** The 0.2.0 goal is to
   *read* existing decks; a reader that silently re-interpreted the
   six-value form, or rejected it (option 2 in #95), would break decks
   written against earlier releases of this tool for no compatibility gain
   — the documented form is added either way.
3. **Documenting the divergence permanently (option 3 in #95) leaves the
   gap open.** A deck writing the documented seven values would still be
   rejected, which is precisely the drop-in-reading failure the epic
   exists to close.

The cost is one clause name with two grammars. It is paid down in the
error: a value list that is neither six nor seven long is a line-numbered
error naming **both** spellings, so a deck that miscounts is told what both
are rather than nudged toward one.

On the status key above, the six-value row is *Supported, extended* rather
than *differs*: the extension shares the documented clause's **name** but
not its **arity**, so it shadows nothing the format documents — a deck
writing the documented seven values always gets the documented reading.
That is the narrow sense in which this is additive, and it is the whole
reason arity is a safe discriminator here rather than a guess.

**What the seven-value form means, exactly.** It is `contact decay_rect`
without the two outward limits, so it is read as one with both `maxcell`s
at the negative "no limit" sentinel: `contact rect (x, y, z, xwidth,
ywidth, xcell, ycell)` *is* `contact decay_rect (x, y, z, xwidth, ywidth,
xcell, ycell, -1, -1)`. That follows from the shapes' own relationship —
`decay_rect` is the `rect` that states its outward decay limits — and it
means the `contact decay_rect` row above describes both: `ceil(width/cell)`
fine cells per axis, the documented decay law `1/(1 − cell/width)` outside,
and `cell` required to be smaller than `width`. `contact_rect_is_a_decay_rect_without_the_limits`
in `fasterhenry-cli/src/inp.rs` asserts the identity, and
`contact_rect_two_spellings_are_one_region` asserts that the six-value
corner form is the seven-value form at `cell = width/2` (2 × 2 cells at
ratio 2), so the two spellings genuinely name one clause.

**The axes.** `xwidth`/`ywidth`/`xcell`/`ycell` are read in the **plane's
own** `p1 → p2` / `p2 → p3` coordinate system, exactly as `contact
decay_rect`'s are — because the documented seven-value form *is* a
`decay_rect` without the outward limits (above), the two cannot disagree on
what frame their shared `xwidth`/`ywidth`/`xcell`/`ycell` are stated in.
Issue #118 settled that question fleet-wide, for `contact point`, `line`,
`decay_rect`, `equiv_rect` and `connection` at once; this clause inherits
the answer rather than needing its own, since it is one of them under a
second name. The six-value corner form's two corners are not lengths and
stay ordinary global coordinates, untouched by that decision. The grammar
question #95 settles — corner pair versus centre and widths — is
independent of it.

**What would reopen this.** Public documentation of a `contact rect` taking
six values (which would collide with the extension and force it out), or a
decision to drop this reader's non-documented extensions wholesale at a
breaking release.

### Decision: `hole user1`…`user7` are rejected permanently (issue #99)

The format documents seven **user-defined** hole generators, `hole user1
(val1, val2, …)` … `hole user7 (…)`. What they cut is a function the user
writes and compiles into the tool; the deck carries only the generator's
number and a bare value list. **Nothing in the deck says what those values
mean** — they are arguments to a function this project does not have and,
under the clean-room rule, may not go looking for.

That makes them unlike every other rejected shape in the table above. A
diagonal `contact trace` is a shape whose *public description disagrees
with itself* (issue #110, decided below) — a statement away, not a model
away. A
`contact initial_grid` was a shape whose *value list* was not pinned down
here — one documented fact away, not a model away, and issue #113 has since
read that fact out of the public description (the decision below). A
`contact circle` is a shape the format does not have at all (issue #109, the
decision below), which is a fact about the format rather than about this
reader. A
`hole user3` is not a shape at all
— no plane model, however general, lets a reader recover a meaning the deck
never wrote down. So the rejection is not deferred work, and the error says
so rather than pointing at a tracking issue that could never close.

**Considered and declined: a predicate hole in the library.** The obvious
alternative (issue #99's option 1) was a further variant on
`fasterhenry::plane::Hole` — `Hole::Predicate(…)`, a caller-supplied
`Fn([f64; 2]) -> bool` over cell centres — giving a *program* the arbitrary
hole a *deck* cannot express. It was declined for three reasons:

1. **The capability already exists, exactly, with no new API.**
   `GroundPlane::mesh` does not depend on `GroundPlane::holes` — holes
   remove cells from the mesh, they never move its edges. So a caller can
   mesh the plane, apply any rule at all to the cell centres it reports, and
   cut each selected cell with a `Hole::Point` at its own centre; a centre
   lies strictly inside its own cell, so each point removes precisely that
   one cell. The composition is exact, not an approximation, and it holds on
   a graded mesh too (both are tested in `fasterhenry/src/plane.rs`, and the
   recipe is a doctest on `Hole`):

   ```rust
   let mesh = plane.mesh()?;
   for i in 0..mesh.nx() {
       for j in 0..mesh.ny() {
           let c = mesh.centre(i, j);
           if my_rule([c[0], c[1]]) {
               plane.holes.push(Hole::Point { at: [c[0], c[1]] });
           }
       }
   }
   ```

2. **The useful form of the variant would cost the type's derives.** A bare
   `fn` pointer keeps `Hole: Copy + PartialEq` but cannot capture anything —
   and a captured parameter list is the whole point of a user hole, so that
   form would not serve the motivating case. The form that does (a boxed or
   `Arc`-wrapped `Fn`) costs `Copy` and `PartialEq` on a type introduced one
   release ago, in exchange for a capability item 1 already provides.

3. **It would be public API with no caller.** The deck reader can never
   construct it (that is this very decision), and there is no in-repo
   program that needs it. `Hole` stays `#[non_exhaustive]`, so if a concrete
   caller does appear the variant can still be added; deferring costs
   nothing that adding it now would save.

**What would reopen this.** Not a deck — no deck can carry the missing
meaning. Only a library caller whose rule is genuinely too slow or too
awkward through item 1: the point-per-cell composition is `O(cells ×
holes)`, so a very large plane whose rule removes a large fraction of its
cells is the case that would justify paying for `Hole::Predicate`. That is a
measured need, not a speculative one, and it is what the `#[non_exhaustive]`
marker is held in reserve for.

### Decision: `contact circle` is not a documented shape (issue #109)

`contact circle` was carried in this table as a *deferred* shape — one whose
argument list had still to be pinned from the public documentation — ever
since the first plane-clause audit. Issue #109 went looking for that
argument list. **There is none, because there is no such clause.**

The `contact` family is not described in the FastHenry user's guide at all:
the guide's one mention of it is a note recommending `contact equiv_rect`
for connections to a nonuniformly discretized plane, which points at a
separate public supplement ("Nonuniformly Discretized Reference Planes in
FastHenry 3.0", M. Kamon, 10 October 1996). That supplement is where every
`contact` clause this table records comes from, and it names them all: the
simple refinement utilities `point`, `line`, `rect` and `decay_rect`; the
contact-*area* utility `equiv_rect`; the grouped `connection` and `trace`,
written in terms of the simple ones; and the `initial_grid` /
`initial_mesh_grid` pair that pre-divides the plane. No disc appears
anywhere in it. `circle` is a **hole** shape (`hole circle (x, y, z, r)`,
from the user's guide's uniform-plane section), and holes and contacts do
not share a shape vocabulary: the same supplement records that these
routines do not support the hole utility at all.

So the earlier "argument list still to be pinned" wording had the situation
backwards, and this row is now rejected the way `hole user1`…`user7` are:
not deferred work, but a name with no meaning to read. The reader keeps
rejecting it by name, on the statement's own line, and the error says which
family each name belongs to so a deck that meant `hole circle` is told
exactly that.

**The geometry was never the obstacle.** Had the clause existed, the mapping
would have been a one-liner on top of what `contact decay_rect` already
does: refine the disc's **bounding square**. That is not an approximation of
a disc on this engine — it is what a disc costs here. The plane mesh is a
tensor product, so a refined x-band spans the plane in y and a y-band spans
it in x; any refinement that covers a disc therefore has the disc's
bounding-square bands, and their crossing is the square. (This is the same
argument the diagonal `contact line` row above rests on.) Refining `4/π` of
the disc's area is the price, the same on any tensor-product mesh, and
`contact decay_rect (x, y, z, 2r, 2r, xcell, ycell, xmaxcell, ymaxcell)`
buys it today — widen each width by its own cell to keep the cells grazing
the rim fine too. `a_discs_bounding_square_is_what_refining_a_disc_costs` in
`fasterhenry-cli/src/inp.rs` asserts both halves of that: every cell inside
the disc honours the requested size, and so does a bounding-square corner
the disc misses.

**What would reopen this.** Public input-format documentation naming a
`contact circle` utility *and* its value list. Then the row becomes a
two-line parser arm over the mapping above, with the padding rule
`contact point` already uses. Nothing about the plane model would change.

### Decision: a diagonal `contact trace` is rejected natively, approximated under compat (issue #110, #157)

*Native mode* keeps the rejection described below: the public description
does not determine the cell size, so the native reader will not guess.
*`--fasthenry-compat`* (issue #157) instead accepts the clause and refines
the trace's bounding box, padded by `3w/2` on every side, to cells
`(w/2)·s^min(|tan θ|, |cot θ|)` on both axes (`s` = `scale_factor`, θ from
`p1 → p2` in the plane's frame), as one `contact rect`. Black-box runs
against self-authored decks show this lands within a few percent of
FastHenry's own staircase refinement (about 0.1-0.4% in L and 0.3-2% in R at
`s = 1`), against about 5% without any refinement. The `min()` deliberately
keeps 30° and 60° mirror-symmetric rather than copying FastHenry's behaviour
past 45°, and the cell is capped at half the box's narrower width. A
line-numbered warning says the refinement approximates FastHenry's staircase.
The word "permanent" in this section's native rationale applies to native
mode only.


A `contact trace` parallel to x or y is supported (its row above): the
public memo *Nonuniformly Discretized Reference Planes in FastHenry 3.0*
(M. Kamon, 1996, section "Grouped contact utilities") writes that case out
line by line, and says `scale_factor` does not affect it. For a trace at
an angle θ to the x axis, the same section gives the cells under the trace
as magnified by `scale_factor` — and states that magnification two ways
that do not agree:

1. As a law: the cell sizes handed to `contact line` are multiplied by
   `scale_factor^|tan θ|`.
2. As prose: the magnification varies continuously from 1 to
   `scale_factor` as the angle goes from 0° to 45° **or from 90° to 45°**.

For θ between 0° and 45° the two agree. For θ between 45° and 90° they do
not: `|tan θ|` exceeds 1 and grows without bound toward 90°, so the law
magnifies the cells by *more* than `scale_factor` — without limit for a
nearly-vertical trace — where the prose keeps the factor between 1 and
`scale_factor` and returns it to 1 at 90°. A reader has to pick one, and
the other is a different mesh. Even below 45° the memo leaves two further
facts unstated that the cell size turns on:

- **Where the side lines go.** The axis-aligned expansion offsets its four
  side lines by `±w/2` and `±3w/2` across the trace; for a diagonal trace
  "across" could mean perpendicular to it or along one axis, and the memo
  shows neither.
- **What cell the lines ask for along the trace.** An axis-aligned trace
  asks for the trace's own length along it — no refinement there. A
  diagonal trace has no axis "along it"; the memo's 45° example gives the
  cell *directly under* the trace (`scale_factor · trace_width / 2`), which
  implies both axes get the magnified across-cell there, but says nothing of
  a 10° trace, where refining the whole bounding box to `~trace_width/2` in
  x is precisely what the utility exists to avoid.

So the cell size a diagonal `contact trace` asks for is not determined by
the public description, and the clean-room rule (`CONTRIBUTING.md`) forbids
settling it from anywhere else. The error therefore does not guess: it names
`contact line (x0, y0, z0, x1, y1, z1, xcell, ycell)`, with which a deck
states the cell it wants outright (on this engine's tensor-product mesh a
diagonal line refines its padded bounding box — see the `contact line` row
— which is also what any reading of the diagonal `contact trace` would
cost here).

**What would reopen this.** A public statement that settles the 45°–90°
law and the diagonal side-line placement — not a deck, since a deck can
only repeat the ambiguous clause.

### Decision: `(n1, n2)` counts `p1 → p2` then `p2 → p3` (issue #113)

`contact initial_grid` and `contact initial_mesh_grid` were rejected by name
until issue #113, for one reason: the pair of counts sets the plane's
*initial* discretization, and reading them the wrong way round would
silently mesh every non-square plane transposed — a deck that still solves,
just at the wrong resolution. Issue #101 declined to guess, which is what
the clean-room rule requires (`CONTRIBUTING.md`: no guessing an undocumented
argument order or row/column convention).

The convention **is** documented, in the same supplement every other
`contact` clause here is read from ("Nonuniformly Discretized Reference
Planes in FastHenry 3.0", M. Kamon, 10 October 1996), read together with the
FastHenry user's guide section on uniformly discretized planes. Three facts
settle it, and this reader implements what they say:

1. **The supplement states the equivalence outright.** Its meshed-planes
   section says that the old-style uniform specification `seg1=10 seg2=12`
   "could be replaced with `file=NONE contact initial_grid (10,12)`". So the
   first value is `seg1` and the second `seg2`, position for position.
2. **The user's guide defines that pair against the edges.** `seg1` is the
   number of segments along the edge from `(x1,y1,z1)` to `(x2,y2,z2)` and
   `seg2` the number along the edge from `(x2,y2,z2)` to `(x3,y3,z3)` — with
   `(seg1+1)·(seg2+1)` nodes, so both are counts of **cells**, not of grid
   lines. Its own reference-plane figure labels the `p1 → p2` edge `seg1`.
   The supplement adds that `p1` is the origin of the plane coordinate
   system, `p1 → p2` its x-direction and `p2 → p3` its y-direction.
3. **The supplement's own worked example only comes out square-celled read
   this way.** Its run-time-discretization example declares a plane whose
   `p1 → p2` edge is 8500 units long and whose `p2 → p3` edge is 13500, and
   meshes it with `contact initial_mesh_grid (34, 54)`. Taking the first
   count along `p1 → p2` gives 8500/34 = 250 and 13500/54 = 250: square
   cells, exactly. Transposed it gives 8500/54 ≈ 157.4 by 13500/34 ≈ 397.1 —
   cells of two different, non-round sizes on a power plane whose designer
   plainly wanted a 250-unit grid.

**The one sentence that reads the other way, and why it does not win.**
Between statements 1 and 2 above, the supplement glosses `(10,12)` as "10 is
the number of rows and 12 the number of columns where a row has a co[n]stant
y value". Rows of constant y stack along y, so read literally — with the
plane's y being `p2 → p3` — that sentence makes the *first* count the
`p2 → p3` one, contradicting the `seg1=10 seg2=12` equivalence in the very
next line of the same paragraph. Something in that sentence is loose, and
the question is only which part.

It is resolved against the sentence, for three reasons. The equivalence
(1) is explicit, positional and testable; the prose is a gloss on what the
figure beside it looks like, and the figure carries no axes, so it fixes
what "row" means on the page but not which plane edge the page's vertical
is. The arithmetic (3) is independent of both and agrees with the
equivalence to the last digit. And the failure modes are not symmetric: a
reader that honours the equivalence reproduces the documented `seg1`/`seg2`
deck exactly, whereas one that honours the gloss silently disagrees with the
supplement's own replacement rule. A deck author who wants no part of this
argument can write `seg1`/`seg2`, which is unambiguous and which this reader
has always taken.

**The checkerboard.** `initial_mesh_grid` marks "every cell that has an even
value for both of its indices where the numbering is from the top left". The
figure of a `(7,6)` meshed grid shows the holes at the 1-based even indices
on both axes — second, fourth and sixth — so the numbering starts at 1, and
a hole may touch the plane's edge when a count is even (the `(7,6)` figure's
sixth column is holed) but never when it is odd. This reader numbers from
the `p1` corner along both axes, `p1` being the origin of the plane
coordinate system by (2) and the corner the user's guide's own plane figure
draws at the top left. That choice is only observable when a count is even:
for an odd count the even indices are symmetric about the plane's centre,
so either end selects the same cells. `the_meshed_initial_grid_*` tests in
`fasterhenry-cli/src/inp.rs` pin both parities, and the transposition guard
(a 5 × 3 grid on a 10 × 6 plane, asserted equal to `seg1=5 seg2=3` and
*unequal* to `seg1=3 seg2=5`) is
`the_initial_grid_is_seg1_and_seg2_and_is_not_transposed`.

**What would reopen this.** Public documentation that pins the row/column
gloss the other way *and* explains away the `seg1`/`seg2` equivalence and
the example's cell arithmetic — all three, since each is independently
sufficient here. Short of that, the mapping stands as implemented.

### Decision: a `contact` clause's `x…`/`y…` pair is in plane coordinates (issue #118)

Until issue #118, this reader applied the `contact` clauses' per-axis
lengths along **global** x and y. That is right for the plane every public
example writes — one whose `p1 → p2` edge runs along global x — and wrong for
a plane whose first edge runs along global y, which this reader has always
accepted (`seg1_and_seg2_follow_the_edges_not_the_axes`). On such a plane an
anisotropic request was silently transposed: a deck that still solves, at the
wrong resolution on each axis. **The reader now maps the pair through the
plane's own axes**, so the five clauses that carry one agree with the rest of
the plane model.

**What the memo says.** The same supplement every other `contact` clause here
is read from ("Nonuniformly Discretized Reference Planes in FastHenry 3.0",
M. Kamon, 10 October 1996) settles it in the clause where it matters most:

1. **It defines the frame.** `p1` "specifies the origin of the plane
   coordinate system"; "the vector from p1 to p2 specifies the x-direction in
   the plane coordinate system. And similarly, the vector from p2 to p3
   specifies the y-direction."
2. **It states `contact point`'s cell sizes in that frame, by name.** Of
   `contact point (1,0,0,0.1,0.2)` it says the point "will be contained in a
   cell of dimensions no bigger than (0.1,0.2) where 0.1 is the width of the
   cell **in the plane coordinate system's x-direction**, and 0.2 is the
   width in the y-direction". This is explicit, not inferred.
3. **The other four clauses are defined as `point`.** The memo builds `line`
   from `point` ("walks along the line, calling the point utility for every
   cell it crosses"), `rect` from `line` ("calls the line utility for a set
   of parallel lines in the x-direction and another set in the y-direction"),
   `decay_rect` from `rect` ("calling the rect utility for gradually larger
   rectangles with gradually larger cell sizes"), and `connection` as exactly
   `decay_rect (x,y,z,xwidth,ywidth,xwidth/ratio,ywidth/ratio,-1,-1)` plus
   `equiv_rect (x,y,z,xwidth,ywidth)` over the same widths. A pair cannot
   change frames as it is passed down that chain, and `connection`'s two
   halves would disagree with each other if `equiv_rect`'s widths were
   global while `decay_rect`'s were not.
4. **It is the convention the plane already uses elsewhere.** `seg1`/`seg2`
   count along `p1 → p2` and `p2 → p3` (the user's guide), and `contact
   initial_grid`/`initial_mesh_grid` likewise (the decision above, issue
   #113). Reading the `contact` pairs globally would have made this one plane
   model use two different frames for its two kinds of per-axis value.

**Coordinates are not relative.** The other half of the question #118 asked:
`p1` being the frame's origin does not make the clauses' points, line ends or
rectangle centres offsets from `p1`. The memo's own `decay_rect` example
places contacts at `(1,0,0)` and `(9,0,0)` on a plane spanning `y = -2 … 2` —
`y = 0` there is the plane's mid-height, the natural place for a two-port
ground plane's contacts, and is *not* a plane-relative coordinate (relative
to `p1 = (0,-2,0)` those contacts would sit on the plane's edge). Its
`equiv_rect` example reads the same way. So every coordinate in a `G`
statement's body stays an ordinary global deck coordinate, checked against
the plane's footprint and slab; only the `x…`/`y…` *lengths* turn with the
plane. `p1` is used as an origin exactly where the memo needs one: for those
two directions, and for the `initial_mesh_grid` cell numbering (above).

**What this changes in practice.** Nothing for a plane whose `p1 → p2` edge
runs along global x — the mapping is the identity there, which is why no
existing test moved. For a plane whose first edge runs along global y, a
clause's `x…` value is now measured along global y. `contact trace` is
unaffected either way: it carries no `x…`/`y…` pair (one `trace_width`, and a
direction taken from its own global ends) and its five-line expansion is
symmetric in the two axes. The documented seven-value `contact rect` is not
listed among the five above only because it is not a sixth clause: it *is* a
`contact decay_rect` under another name (issue #95), so it turns with the
plane exactly as `decay_rect` does, through the same code. This reader's
own six-value corner-spelled `contact rect` genuinely is unaffected — its
two corners are global coordinates, not a plane-relative length pair.

**What would reopen this.** Public documentation stating one of these five
clauses' pairs in global x/y *despite* the plane coordinate system the same
document defines — or a worked example that only comes out right read
globally. Statement 2 above is a direct quotation about the clause the other
four are built from, so short of that the mapping stands as implemented.
`contact_cell_sizes_follow_the_plane_axes_not_the_global_ones` in
`fasterhenry-cli/src/inp.rs` pins all five clauses (plus the documented
`contact rect`, which is `decay_rect` under another name) against a rotated
plane, `a_square_contact_request_is_orientation_independent` pins the
isotropic no-op (`contact trace` included), and
`a_rotated_planes_decay_limit_error_names_the_decks_own_axis` pins that the
per-axis error messages name the axis the deck wrote.

### Decision: a hole or contact rectangle off its plane is accepted with a warning (issue #105)

Every `hole` / `contact` clause's `z` has always been checked against the
plane's slab, but until issue #105 a `hole rect`, `hole point` or `hole
circle` whose **xy** lay wholly outside the plane's footprint was silently
accepted and removed nothing — the mesh quietly differed from the one the
deck asked for, typically because of a coordinate typo or a wrong `.units`
scale. The rule now, per shape:

| Clause | Wholly outside the footprint means | What happens |
|---|---|---|
| `hole point (x, y, z)` | the point is outside the closed footprint | accepted (removes nothing) + warning |
| `hole circle (x, y, z, r)` | the closed disc does not intersect the closed footprint — a centre off the plane is **not** enough if the radius still reaches over an edge or corner | accepted (removes nothing) + warning |
| `hole rect (x1, y1, z1, x2, y2, z2)` | the two rectangles do not intersect — partial overhang, or merely touching the boundary, is not disjoint | accepted (removes nothing) + warning |
| `contact rect` (either spelling), `contact decay_rect` | the contact's rectangle does not intersect the footprint | accepted (refines nothing; the region is dropped) + warning |
| `contact connection N<name> (…)` | its `decay_rect` half as above | the clause's tie half keeps its existing error first (below), so it never parses with a disjoint decay region |

The warning is a diagnostic, not an error: a deck that parsed before still
parses to the same mesh, and stdout is unchanged. It names the clause, the
ground plane and the clause's **physical** line — the continuation (`+`)
line it is written on, not the line its `G` statement starts on — and there
is exactly one per offending clause, however many share a statement or a
line. The command-line binary prints each on stderr as `warning: line N:
…`; the library never prints: `inp::parse_reporting` /
`inp::parse_with_options_reporting` and `read_inputs_reporting` /
`read_inputs_reporting_with` return the warnings, while `parse`,
`parse_with_options`, `read_inputs` and `read_inputs_with` keep their
signatures and discard them (as `run` does `run_reporting`'s).

Why a warning rather than an error: the clause is not malformed, and
turning an accepted clause into a rejected one would break decks that
parse today; a warning catches the typo without that cost. It is
deliberately **only** about disjointness from the footprint. A clause inside
the footprint that happens to change nothing — a circle too small to catch a
cell centre, a hole inside another hole, a contact the background mesh
already meets — does not warn: those are legitimate decks, and "warn
whenever the final mesh is unchanged" would be different semantics.

**Touching contacts (issue #134).** A `contact rect` / `contact decay_rect` that meets the footprint only at its boundary — an exact touch, or an overlap no wider than `fasterhenry::plane::contact_slack` on some axis: the larger of a shape-relative `1e-9 × span` and a coordinate-rounding `32 × f64::EPSILON × max(|lo|, |hi|)` (scale-aware, not a fixed number of metres; the two are kept separate so a small plane translated far from the origin still keeps genuine partial overlaps) — carries no two-dimensional region, and is treated exactly like a disjoint one: dropped with one line-numbered warning (`… only touches the boundary …`) on its own clause line. Before this, a rounding-sized overlap (`11e-3 - 1e-3` lands just below the plane edge) reached the mesher and failed assembly on line 0 with a zero-length segment. The library rejects the same overlaps as `PlaneError::ContactOutsideFootprint`; genuine partial overlaps still refine at the declared resolution. Holes keep their closed-boundary behaviour.

Two consequences worth stating:

- **A wholly-disjoint `contact rect` / `contact decay_rect` used to be an
  error** — not from this reader but from the plane library at assembly,
  reported on line 0 ("contact region … lies outside the plane footprint").
  Such a region refines nothing, so it is now dropped with the warning
  instead, on its own line, consistent with the holes. No deck that parsed
  before changes meaning. A rectangle that only *touches* the footprint
  (zero overlap area) is not disjoint by this rule, so it still reaches the
  library and keeps that existing error.
- **The stricter clauses stay errors.** `contact point`, `contact line` and
  `contact trace` name a locus that must lie on the plane, and an off-plane
  end remains a line-numbered error; a `contact equiv_rect` (or the tie half
  of a `contact connection`) whose node is off the plane, or that covers no
  live cell, remains an error too. The `.hole` / `.contact` extension
  directives are unchanged.

`fasterhenry-cli/src/inp.rs`'s tests `disjoint_hole_point_warns_on_its_own_line`,
`disjoint_hole_circle_warns_on_its_own_line`,
`disjoint_hole_rect_warns_on_its_own_line`,
`disjoint_contact_rects_warn_and_are_dropped`,
`disjoint_contact_decay_rect_warns_and_is_dropped`,
`one_warning_per_disjoint_clause_with_its_physical_line`,
`shapes_meeting_the_footprint_do_not_warn`,
`in_footprint_clauses_that_change_nothing_do_not_warn` and
`stricter_off_plane_errors_are_unchanged` pin the rule, and
`a_hole_off_its_plane_solves_with_a_line_numbered_warning` in
`fasterhenry-cli/tests/cli.rs` pins the command-line behaviour.

## `.couples`

| Status | Notes |
|---|---|
| Supported, extended | Not a documented FastHenry directive at all — a `fasterhenry`-specific addition for truncating mutual-inductance coupling between segment groups (see `fasterhenry::coupling`). Included here only so the name is accounted for: it does not collide with, and is not a variant of, any name the public format documents. |

## Directives outside the deck subset

Anything else — including directives that configure FastCap's multipole
solver (this engine does not use that algorithm, so such parameters have no
equivalent here) — is rejected with a line-numbered error naming the
directive (`'.<name>' is not in the deck subset`) rather than silently
ignored. No such directive surfaced during this audit that is not already
tracked by a sibling issue in the epic (#71, #80); nothing new is filed
as a result of this table beyond what is listed below.

## Follow-ups filed

- Issue #83: reading genuine third-party decks whose first line is prose
  (not `.title`) — resolved by the opt-in `--fasthenry-compat` mode; see
  "Deck framing" above.
- Issue #95: `contact rect`'s argument list, which this reader spelled as
  two opposite corners where the documented form is a centre, full widths
  and cell sizes (found while implementing `contact decay_rect` for #80).
  Since **implemented**: the reader now takes both, told apart by value
  count — seven values are the documented centre/widths/cell form (exactly
  `contact decay_rect` with no outward limits), six keep the corner meaning
  as an extension, and any other count is a line-numbered error naming
  both. See "Decision: `contact rect` reads both spellings, told apart by
  value count" above for why accepting both beat migrating to the
  documented form alone or documenting the divergence permanently.
- Issues #99, #100, #101: the hole and contact shapes #80 and #98 left
  rejected, split by the model change each needs — user-defined holes, the
  `contact` refinement primitives, and the named-equipotential /
  initial-grid contact forms. (Issue #98, non-rectangular holes, has since
  been implemented — see its own `hole point (x, y, z)` / `hole circle (x,
  y, z, r)` row above. Issue #99 has since been *decided* rather than
  implemented: `hole user1`…`user7` are permanently rejected, for the
  reasons in "Decision: `hole user1`…`user7` are rejected permanently"
  above. Issue #100 has since implemented `contact point` and `contact
  line` — see their row above — and moved `contact circle` and `contact
  trace` to issues #109 and #110. Issue #109 has since been *decided*
  rather than implemented, like #99: `contact circle` is not a documented
  shape at all, so it is permanently rejected — see "Decision: `contact
  circle` is not a documented shape" above. Issue #110 has since implemented
  `contact trace` along x or y and *decided* the diagonal case as a
  permanent rejection — see its rows above and "Decision: a diagonal
  `contact trace` is rejected". Issue #101 has since been implemented for
  `contact equiv_rect` and `contact connection` — their own rows above —
  and filed #113 for the two initial-grid forms it deliberately left
  rejected.)
- Issue #113: the row/column convention `contact initial_grid` and
  `contact initial_mesh_grid` turn on, which the public description this
  audit is written from did not settle for #101. Since **implemented**: the
  convention was read out of that description after all — see "Decision:
  `(n1, n2)` counts `p1 → p2` then `p2 → p3`" above for the evidence and for
  the one sentence of it that reads the other way — so both clauses now have
  supported rows in the table.
- Issue #122: `file=` on a plane statement, which the nonuniform-plane
  description defines as an *input* (the discretization hierarchy file,
  `NONE` for none) rather than the output option this reader's rejection
  message used to call it — and whose `file=NONE` form is what the public
  `contact initial_grid` example pairs the clause with. Found while
  implementing #113. Since **implemented**: `file=NONE` is accepted as the
  no-op it is, since "no hierarchy file, discretize from the statement's own
  clauses" is the only case this reader has, so the documented
  `file=NONE contact initial_grid (10,12)` line now reads as written; a
  *named* hierarchy file is still rejected, now as the unread input it is.
  See the two `file=` rows above.
