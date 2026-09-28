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
| Directive keywords case-insensitive (`.UNITS` = `.units`) | Supported | Matched via `to_ascii_lowercase()`. |
| Node/element names case-sensitive | Supported | `N1` and `n1` are different nodes; every reference (`.external`, `E` endpoints, `.equiv`) is matched by exact string equality. |
| `.end` required | Supported, differs | A deck without `.end` is rejected. FastHenry decks conventionally end with `.end`, but this reader treats its absence as an error rather than an implicit end-of-file — consistent with the "no default swallows a mistake" design already applied to `.units`. |
| Content after `.end` | Supported, differs | Comments and blank lines after `.end` are fine (dropped before the `.end` state is even consulted, same as elsewhere in the file); another directive/node/segment line after `.end` is an error. Multi-deck-in-one-file constructs some formats allow after a terminator are out of scope: this reader parses exactly one deck per input. |

## `.units`

| Field | Status | Notes |
|---|---|---|
| `.units <unit>` | Supported, differs (mandatory) | The engine takes the presence of `.units` as mandatory rather than defaulting silently — a deliberate safety choice, so a missing unit line can never scale every length (and therefore every impedance) by a factor of 10 or 100 without a diagnostic. A deck without `.units` is rejected with a line-numbered error naming the missing directive rather than assuming a default unit. |
| Full documented unit list: `km`, `m`, `cm`, `mm`, `um`, `in`, `mils` | Supported | Case-insensitive (issue #70); `mil` is also accepted as a synonym for `mils`. |

## `.default`

| Field | Status | Notes |
|---|---|---|
| `.default x= y= z= w= h= nwinc= nhinc= rw= rh= wx= wy= wz= sigma=` | Supported | Applies to every `N`/`E` line parsed after it; not retroactive. Requires `.units` first (lengths need a scale factor before they can be stored). |
| `.default rho=` | Supported | Issue #70. Per deck unit, the exact reciprocal of `sigma=`; must be positive. Naming both `sigma=` and `rho=` on one line is a line-numbered error; a later per-line value in either form overrides a `.default` in either form. |

## Nodes (`N`)

| Field | Status | Notes |
|---|---|---|
| `N<name> x= y= z=` | Supported | A coordinate missing from both the line and `.default` is a line-numbered error naming which coordinate and which node. |

## Segments (`E`)

| Field | Status | Notes |
|---|---|---|
| `E<name> N<a> N<b> w= h= sigma=\|rho=` | Supported | `w`/`h` are stored as magnitudes (`abs()`); a missing `w`, `h`, or conductivity (`sigma`/`rho`, line or `.default`) is a line-numbered error naming the segment and the field. |
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
| `G<name> x1= y1= z1= x2= y2= z2= x3= y3= z3=` (three corner points) | Supported, differs | The plane must be axis-aligned and parallel to the xy plane (`z1 = z2 = z3`, each edge along x or y): this engine's plane model is an axis-aligned rectangle. A tilted or rotated plane is rejected by name rather than squared off. The corner points give the plane's mid-thickness surface, as a segment's nodes give its axis. |
| `thick=` | Supported | Plane thickness. |
| `seg1=`, `seg2=` | Supported, differs | Become the background cell counts of this engine's own cell-centre PEEC mesh, not FastHenry's panel mesh: equal counts mean equal resolution, not an identical node set. |
| `sigma=` | Supported | Per deck unit, falling back to the `.default` conductivity (`.default sigma=` or `.default rho=`). |
| `nhinc=` | Supported | Filaments through the plane's thickness. |
| `rho=` | Supported | Issue #88. Per deck unit, the exact reciprocal of `sigma=` and accepted on the corner-point statement itself (continuation lines included); naming both `sigma=` and `rho=` on one statement is a line-numbered error, as it is elsewhere. |
| `rh=`, `segwid1=`/`segwid2=`, `relx=`/`rely=`/`relz=`, `file=` | Not supported | Each rejected by name with the reason and the alternative (plane filaments are uniform; bar widths follow the cells; name in-plane nodes instead; no output-file option). Nothing on a `G` statement is silently ignored. |
| In-plane node `N<name> (x, y, z)` | Supported | An ordinary deck node belonging to the plane; a reference to it lands on the nearest live cell-centre node of that plane. |
| `hole rect (…)`, `contact rect (…)` | Supported, differs | Map onto the plane model's rectangular hole and contact region; the redundant `z` coordinates are checked against the plane's slab. An inline `contact rect` uses 2 × 2 fine cells at ratio 2 — use `.contact` to choose other values. Both take **two opposite corners** here. That is right for `hole rect` and a divergence for `contact rect`, whose documented form is a centre, full widths and cell sizes like the other `contact` shapes'; issue #95 tracks reconciling it. |
| `contact decay_rect (x, y, z, xwidth, ywidth, xcell, ycell, xmaxcell, ymaxcell)` | Supported, differs | Issue #80. Maps onto `fasterhenry::plane::ContactRegion`: centre and full widths give the rectangle, `ceil(width/cell)` per axis gives the fine cells, and the documented decay law `1/(1 − cell/width)` gives that axis's growth ratio. `cell` must be smaller than `width` (the documentation's own `r0 < 1`). The differences are both in the outward limit: this engine's grading levels off at the plane's **background cell** rather than at `maxcell`, so a positive `maxcell` **finer** than the background cell is rejected by name (raise `seg1`/`seg2` instead of shipping a quietly coarser mesh), while one at or above it never binds; a negative `maxcell` — the sentinel the documented `contact connection` shorthand expands to — asks for no limit. The resulting mesh is this engine's own graded cell-centre mesh, not FastHenry's cell subdivision, so equal cell sizes mean equal resolution, not an identical node set. |
| `hole point (x, y, z)`, `hole circle (x, y, z, r)` | Supported | Issue #98. Map onto `fasterhenry::plane::Hole::Point` and `Hole::Circle`, widening the plane's hole model beyond the axis-aligned rectangle. A point removes exactly the one cell whose own extent (edges included) contains it; a point landing exactly on a shared cell edge or corner is the documented tie and removes every cell touching it, rather than guessing a single winner. A circle removes every cell whose centre lies at or inside its radius `r` — a centre exactly on the circle counts (a closed boundary, unlike `hole rect`'s open one, since there is no prior rectangle behaviour to match). Both check their own `z` against the plane's slab like every other hole/contact clause. |
| `hole user1 (…)` … `hole user7 (…)` | Not supported, permanent | Issue #99. Rejected by name on the statement's own line, and the rejection is final rather than deferred — see the decision below. The error names the alternatives: the declarative shapes above, and `fasterhenry::plane::GroundPlane::mesh` + `fasterhenry::plane::Hole::Point` for a shape none of them describe. |
| Other hole/contact shapes (`contact point`, `contact line`, `contact circle`, `contact trace`, `contact equiv_rect`, `contact connection`, `contact initial_grid`, `contact initial_mesh_grid`) | Deferred | Issues #80 and #98 took `decay_rect`, `point` and `circle`; the rest stay rejected by name, on the statement's own line, rather than approximated by a rectangle. Tracked as issues #100 (the `contact` refinement primitives) and #101 (the named-equipotential and initial-grid forms). |
| `G<name> x1 y1 z1 x2 y2 z2 t [nx=] [ny=] [nhinc=] [sigma=\|rho=]`, `.hole`, `.contact` | Supported, extended | This project's own shorthand plane form and its separate refinement directives — not documented FastHenry syntax. Told apart from the corner-point form by the first token after the plane name, so a deck may mix the two; both build the same plane. |

### Decision: `hole user1`…`user7` are rejected permanently (issue #99)

The format documents seven **user-defined** hole generators, `hole user1
(val1, val2, …)` … `hole user7 (…)`. What they cut is a function the user
writes and compiles into the tool; the deck carries only the generator's
number and a bare value list. **Nothing in the deck says what those values
mean** — they are arguments to a function this project does not have and,
under the clean-room rule, may not go looking for.

That makes them unlike every other rejected shape in the table above. A
`contact line` or a `contact initial_grid` is a *shape this reader cannot
represent yet*: its meaning is public, and issues #100/#101 track the plane
model changes that would represent it. A `hole user3` is not a shape at all
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
- Issue #95: `contact rect`'s argument list, which this reader spells as two
  opposite corners where the documented form is a centre, full widths and
  cell sizes (found while implementing `contact decay_rect` for #80).
- Issues #99, #100, #101: the hole and contact shapes #80 and #98 left
  rejected, split by the model change each needs — user-defined holes, the
  `contact` refinement primitives, and the named-equipotential /
  initial-grid contact forms. (Issue #98, non-rectangular holes, has since
  been implemented — see its own `hole point (x, y, z)` / `hole circle (x,
  y, z, r)` row above. Issue #99 has since been *decided* rather than
  implemented: `hole user1`…`user7` are permanently rejected, for the
  reasons in "Decision: `hole user1`…`user7` are rejected permanently"
  above.)
