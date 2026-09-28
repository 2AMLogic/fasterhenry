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

## Deck framing

| Rule | Status | Notes |
|---|---|---|
| First line is a title, always ignored | Supported, differs | This reader has no implicit first line. `.title <text>` is an explicit, additive directive instead — kept deliberately: this parser's whole design rejects anything it cannot place (unknown fields, unknown directives, a missing `.units`) rather than silently reinterpreting a line, and unconditionally discarding line 1 is exactly the silent-reinterpretation failure mode the rest of the reader avoids — a deck that opens with `.units m` instead of a title line would have its unit directive silently swallowed. Reading genuine third-party decks (whose first line is prose, not a directive) needs a real compatibility mode, not a table entry; tracked as follow-up issue #83 rather than fixed here, since it is larger than this issue's scope. |
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
| `hole rect (…)`, `contact rect (…)` | Supported | Map onto the plane model's rectangular hole and contact region; the redundant `z` coordinates are checked against the plane's slab. An inline `contact rect` uses 2 × 2 fine cells at ratio 2 — use `.contact` to choose other values. |
| Other hole/contact shapes (`point`, `circle`, `decay_rect`, `trace`, `initial_*`, `equiv_*`, `user1…user7`) | Deferred | Issue #80; rejected by name rather than approximated by a rectangle. |
| `G<name> x1 y1 z1 x2 y2 z2 t [nx=] [ny=] [nhinc=] [sigma=\|rho=]`, `.hole`, `.contact` | Supported, extended | This project's own shorthand plane form and its separate refinement directives — not documented FastHenry syntax. Told apart from the corner-point form by the first token after the plane name, so a deck may mix the two; both build the same plane. |

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
  (not `.title`), as a real compatibility mode rather than a documented
  difference — see "Deck framing" above.
