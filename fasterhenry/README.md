# fasterhenry

A clean-room, MIT-licensed Rust engine for **PEEC** (partial-element equivalent
circuit) extraction of resistance and inductance of 3-D conductor geometries:
spirals, busbars, bond wires, on-chip interconnect. FastHenry's problem class,
none of its code.

> **Status: 0.1, early release.** Following semver for `0.x`, a minor release
> (`0.2`, `0.3`, …) may make breaking API changes; patch releases stay
> compatible. Changes are recorded in `CHANGELOG.md`.

## What it does

- **Geometry → filaments → `Z(ω)`.** Nodes and rectangular-section segments
  are cut into filaments (uniform, surface-graded, or graded to the skin
  depth at a frequency of interest), assembled into a loop-basis mesh
  system, and solved for the port impedance matrix over a frequency sweep.
- **Ground planes** as meshed conductors, with rectangular holes and graded
  contact regions under via landings.
- **Coupling truncation**: declare which groups of segments couple and skip
  the mutual inductance of the rest.
- **Two solvers.** The default is a dense complex LU, parallel across all
  cores with SIMD-batched kernels. For large problems, `IterativeSystem`
  solves the same system matrix-free: restarted GMRES on a precorrected-FFT
  (`PfftOperator`) evaluation of `L·x`, in near-linear time and memory.
- **Validated** against independent oracles — PyPEEC and the Greenhouse
  closed forms on a spiral fixture (≤ 0.5 %), plus skin-effect and
  ground-plane references; see
  [`docs/validation.md`](https://github.com/2AMLogic/fasterhenry/blob/main/docs/validation.md).

## Use

```toml
[dependencies]
fasterhenry = "0.1"
```

A 1 mm copper bar with a port across its ends, swept from DC to 1 GHz (SI
units throughout):

```rust
use fasterhenry::{solve, Discretization, Geometry, Node, Port, SegmentDef, Subdivision};

let mut geometry = Geometry::new();
let a = geometry.add_node(Node::new(0.0, 0.0, 0.0))?;
let b = geometry.add_node(Node::new(1e-3, 0.0, 0.0))?;
geometry.add_segment(SegmentDef::new(a, b, 100e-6, 20e-6, 5.8e7))?;

let result = solve(
    &geometry,
    &[Port::new(a, b)],
    &Discretization::Uniform(Subdivision::new(5, 3)),
    &[0.0, 1e6, 1e9],
)?;
println!("R(DC) = {} Ω", result.resistance(0)[(0, 0)]);
println!("L(1 MHz) = {:?} H", result.inductance(1).map(|l| l[(0, 0)]));
```

`SweepResult` serializes to JSON. API documentation:
<https://docs.rs/fasterhenry>. For a command-line tool that reads FastHenry
`.inp` decks, see the
[`fasterhenry-cli`](https://crates.io/crates/fasterhenry-cli) crate.

## Method

Ruehli's PEEC formulation; the approach of Kamon, Tsuk and White (*FASTHENRY:
a multipole-accelerated 3-D inductance extraction program*, IEEE Trans. MTT
42(9), 1994); Grover/Rosa closed forms for parallel filament partial
inductances; numerical quadrature for arbitrary orientation; the
precorrected-FFT method of Phillips and White for acceleration. Implemented
from the papers, not from any existing program's source.

## Stack

Rust (stable), `nalgebra` + `simba`/`wide` for portable SIMD, `rayon` for
parallel matrix fill, `num-complex`, and `rustfft` (pure Rust) for the
precorrected-FFT operator. No BLAS/LAPACK, no C dependencies, and
`#![forbid(unsafe_code)]`.

## License

MIT — see `LICENSE`. Changes are recorded in `CHANGELOG.md`. Source,
issues, and the contribution rules (including the clean-room rule) live at
<https://github.com/2AMLogic/fasterhenry>.
