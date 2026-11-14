# JSON problem input format

The CLI reads a problem from one file, and picks the format **by file
extension**: `.inp` and `.fh` are parsed as FastHenry decks, **any other
extension** as the JSON problem document described here
(`fasterhenry-cli/src/lib.rs`, `read_inputs_reporting_with`).

That is worth stating first because it is the easiest mistake to make: a JSON
document named `problem.inp` is parsed as a deck and fails with a deck
syntax error, not a JSON one. Name it `.json` (or anything that is not
`.inp`/`.fh`).

Both spellings produce the same `Problem` value — a deck is converted into one
after parsing — so the deck dialect is not a different data model, just a
different surface. For the deck, see
[`fasthenry-compat.md`](fasthenry-compat.md); for what comes back out, see
[`output-formats.md`](output-formats.md).

**Units are SI throughout**: metres, siemens per metre, hertz. There is no
`.units`-style scale factor in the JSON form.

## The document

A problem document is a JSON object with five keys:

| key | type | required | meaning |
|---|---|---|---|
| `geometry` | object | **yes** | the nodes and the segments connecting them |
| `ports` | array | **yes** | terminal pairs, in the row/column order of the result matrix |
| `discretization` | tagged object | **yes** | how each segment is cut into filaments |
| `coupling` | object | no — defaults to coupling every pair | which conductor groups are magnetically coupled |
| `frequencies_hz` | array of number | **yes** | the frequency sweep, hertz |

**Unknown keys at this level are rejected** — a typo is an error, not a
silently ignored field. The same holds for `geometry`, for each node and
segment, for each port, and for `coupling`; it does **not** hold inside a
`discretization` payload (see the boundary noted under
[Discretization](#discretization) below).

`coupling` is the only optional key, and it is left out again when a problem
is serialized back to JSON if it carries no group tags and truncates nothing
— so a document written before the key existed round-trips unchanged.

An empty `"geometry": {}` parses — both of its keys default to empty — as do
empty `ports` and `frequencies_hz` arrays. Such a document is rejected by the
*solve* rather than the parse: `no ports` or `no frequencies` when either list
is empty, and a port naming a node the geometry does not have is reported
against the port's index.

## `geometry`

```
"geometry": { "nodes": [ … ], "segments": [ … ] }
```

Both keys default to the empty array.

Geometry is **validated as it is parsed**: a non-finite coordinate, a segment
referring to a node that does not exist, a zero-length segment, a
non-positive or non-finite width, height or conductivity, and a width
direction that is zero or parallel to the segment are all rejected at parse
time, with the index of the offending node or segment in the message. Every
geometry that parses is therefore a valid one.

### `nodes[]`

A point where segments begin, end, and join.

| field | type | required | meaning |
|---|---|---|---|
| `x`, `y`, `z` | number | **yes** | coordinates, metres |

Two segments meet because they name the **same node index** — that, and only
that, is what records a connection.

### `segments[]`

A straight conductor of rectangular cross-section between two nodes. Current
flows from `a` to `b`.

| field | type | required | meaning |
|---|---|---|---|
| `a`, `b` | integer | **yes** | indices into `nodes` (0-based) |
| `width` | number | **yes** | cross-section extent along the width direction, metres |
| `height` | number | **yes** | cross-section extent along the height direction, metres |
| `sigma` | number | **yes** | conductivity, S/m (copper ≈ `5.8e7`) |
| `width_dir` | `[x, y, z]` | no | explicit cross-section orientation |

`a` and `b` are bare integers, not objects: `{ "a": 0, "b": 1, … }`.

`width_dir` need not be normalized nor exactly perpendicular to the segment —
its component perpendicular to the centreline is what is used. Omit it to get
the default orientation; it is likewise left out again when a segment that
does not set it is serialized back to JSON.

## `ports`

Each port is the terminal pair at which an impedance is extracted. The array
order is the row and column order of the result matrix.

| field | type | required | meaning |
|---|---|---|---|
| `positive` | integer | **yes** | node index at which the port current enters |
| `negative` | integer | **yes** | node index at which it leaves |
| `name` | string | no | label, carried through to the results |

Swapping `positive` and `negative` flips the sign of that port's off-diagonal
impedances and leaves its self-impedance unchanged. At least one port is
required — by the solve, not by the parse.

## `discretization`

Exactly one key, naming the scheme; its value is the payload. The five
schemes are:

| key | payload | meaning |
|---|---|---|
| `uniform` | `{"nw": 3, "nh": 2}` | the same uniform `nw × nh` filament grid on every segment |
| `per_segment` | `[{"nw": 3, "nh": 2}]` | one uniform grid per segment, **in segment order** — one entry per segment |
| `graded` | `{"nw": 4, "nh": 4, "ratio": 2.0}` | the same surface-graded grid on every segment |
| `skin_depth` | `{"frequency_hz": 1e9, "target_skin_depths": 0.5, "ratio": 2.0, "max_per_axis": 32}` | graded, with counts chosen per segment from the skin depth at a frequency of interest |
| `per_segment_graded` | `[{"nw": 4, "nh": 4, "width_ratio": 2.0, "height_ratio": 1.5}]` | one grid per segment, each with its own ratio per axis — one entry per segment |

So, for example, `"discretization": { "uniform": { "nw": 3, "nh": 2 } }`.

- `nw` and `nh` are filament counts across the width and the height.
- `ratio` (and `width_ratio` / `height_ratio`) is the ratio of adjacent
  filament extents, coarsening **inward from both faces**: it must be finite
  and at least 1. A ratio of exactly `1.0` reproduces the uniform grid of the
  same counts bit for bit.
- `target_skin_depths` is the thickness of the surface filaments in skin
  depths; values below 1 resolve the current profile within the skin, and
  `0.5` is a reasonable default. At `frequency_hz` of 0 the skin depth is
  infinite and every segment becomes a single filament.
- `max_per_axis` bounds the filament count per axis. It caps **cost**, not
  numerical safety: a grid that is selected and then turns out to be
  numerically unusable is reported as an error, never silently coarsened.
  **It has no default in JSON** — unlike the Rust constructor, which fills in
  32 — so it must be written out explicitly in a `skin_depth` payload.
- `per_segment` and `per_segment_graded` are **positional**: entry *i*
  applies to segment *i* of `geometry.segments`.

Unlike every other object in the document, a `discretization` payload does
**not** reject unknown keys. `{"uniform": {"nw": 3, "nh": 2, "typo": 1}}` is
accepted and the stray key ignored, where the same typo at the top level of
the document is an error. Do not rely on a misspelled grading parameter being
caught for you.

Grading ratios are checked when the problem is **solved**, not when it is
parsed, so an out-of-range `ratio` surfaces as a solve error naming the
segment, axis, counts and ratio.

## `coupling`

Which groups of conductors are magnetically coupled. Both keys are optional.

| field | type | default | meaning |
|---|---|---|---|
| `segment_groups` | array of string | `[]` | the group each segment belongs to, **in segment order** |
| `couples` | array of `[a, b]` pairs | absent | the cross-group pairs whose mutual inductance is kept |

**An absent `coupling` — and an absent `couples` — couples every pair**, and
ignores the group names entirely. Truncation starts only once `couples` is
present. A group is always coupled to itself, so `"couples": []` keeps each
group's internal mutual inductance and drops every cross-group term.

Segments past the end of `segment_groups`, and entries that are the empty
string, belong to the default group.

Checked when the problem is solved: a `couples` entry naming a group that no
segment belongs to is rejected as a typo rather than treated as a no-op, as
is a `segment_groups` longer than the segment count. A **non-transitive**
declaration — two groups reachable only through a third — is also rejected,
because it can make the inductance matrix indefinite and the solve
unphysical; declare the whole clique.

Truncating a coupling is a physical approximation, not a free speed-up. The
rules of thumb for when it is safe, and the warnings emitted for pairs that
are too close to justify it, are in the `fasterhenry::coupling` module
documentation.

## `frequencies_hz`

The sweep, in hertz, in the order the results are reported. `0.0` is a valid
entry and means DC — the impedance is then exactly real.

The CLI's `--freq FMIN_HZ FMAX_HZ NDEC` replaces this list for one run,
log-spaced at `NDEC` points per decade, without editing the document.

## Ground planes are not expressible in JSON

**A ground plane cannot be declared in a JSON problem document.** There is no
`planes` key, and the plane specification type carries no JSON
representation at all — its extent, cell counts, holes, contact regions and
equipotentials have no serialized form.

An *already-meshed* plane is representable, because a meshed plane is only
nodes and segments, but writing one out by hand is impractical. To use a
ground plane, either:

- write an `.inp` deck and use `G` with `.hole` / `.contact` — see
  [`fasthenry-compat.md`](fasthenry-compat.md) § "Ground planes"; a deck's
  plane is expanded into ordinary nodes and segments while it is parsed, so
  it never needs a JSON form; or
- build the problem through the library API with
  `fasterhenry::plane::GroundPlane`.

## A minimal example

A 1 mm copper trace, 100 µm × 35 µm, excited between its two ends and swept
from DC to 1 GHz:

```json
{
  "geometry": {
    "nodes": [
      { "x": 0.0, "y": 0.0, "z": 0.0 },
      { "x": 1e-3, "y": 0.0, "z": 0.0 }
    ],
    "segments": [
      { "a": 0, "b": 1, "width": 1e-4, "height": 3.5e-5, "sigma": 5.8e7 }
    ]
  },
  "ports": [{ "positive": 0, "negative": 1 }],
  "discretization": { "uniform": { "nw": 3, "nh": 2 } },
  "frequencies_hz": [0.0, 1e6, 1e9]
}
```

## A fuller example

A three-segment coil folded back on itself and, 50 mm away, a separate
one-segment sense trace. Filament counts are chosen from the skin depth at
1 GHz, the mutual inductance between the two is truncated — the groups
`coil` and `sense` are coupled to themselves but not to each other — and
the sense segment is given an explicit cross-section orientation:

```json
{
  "geometry": {
    "nodes": [
      { "x": 0.0, "y": 0.0, "z": 0.0 },
      { "x": 2e-3, "y": 0.0, "z": 0.0 },
      { "x": 2e-3, "y": 2e-4, "z": 0.0 },
      { "x": 0.0, "y": 2e-4, "z": 0.0 },
      { "x": 0.0, "y": 5e-2, "z": 0.0 },
      { "x": 2e-3, "y": 5e-2, "z": 0.0 }
    ],
    "segments": [
      { "a": 0, "b": 1, "width": 5e-5, "height": 1.8e-5, "sigma": 5.8e7 },
      { "a": 1, "b": 2, "width": 5e-5, "height": 1.8e-5, "sigma": 5.8e7 },
      { "a": 2, "b": 3, "width": 5e-5, "height": 1.8e-5, "sigma": 5.8e7 },
      {
        "a": 4,
        "b": 5,
        "width": 5e-5,
        "height": 1.8e-5,
        "sigma": 5.8e7,
        "width_dir": [0.0, 0.0, 1.0]
      }
    ]
  },
  "ports": [
    { "positive": 0, "negative": 3, "name": "coil" },
    { "positive": 4, "negative": 5, "name": "sense" }
  ],
  "discretization": {
    "skin_depth": {
      "frequency_hz": 1e9,
      "target_skin_depths": 0.5,
      "ratio": 2.0,
      "max_per_axis": 16
    }
  },
  "coupling": {
    "segment_groups": ["coil", "coil", "coil", "sense"],
    "couples": []
  },
  "frequencies_hz": [0.0, 1e8, 1e9]
}
```

Both examples above are extracted from this file and parsed as problem
documents by a test (`fasterhenry-cli/tests/input_format_doc.rs`), so they
cannot drift away from the schema.
