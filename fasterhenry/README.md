# fasterhenry

A clean-room, MIT-licensed Rust engine for **PEEC** (partial-element equivalent
circuit) extraction of resistance and inductance of 3-D conductor geometries:
spirals, busbars, bond wires, on-chip interconnect. FastHenry's problem class,
none of its code.

> **Status: 0.0.x, pre-release.** The API is unstable and will change without
> notice between patch versions. The solver is the dense M0 core; acceleration
> (FFT/FMM) and skin/proximity refinement come later. Pin an exact version if
> you depend on it.

## Use

```toml
[dependencies]
fasterhenry = "0.0.1"
```

```rust
println!("fasterhenry {}", fasterhenry::VERSION);
```

API documentation: <https://docs.rs/fasterhenry>. For the command-line tool,
see the [`fasterhenry-cli`](https://crates.io/crates/fasterhenry-cli) crate.

## Method

Ruehli's PEEC formulation; the approach of Kamon, Tsuk and White (*FASTHENRY:
a multipole-accelerated 3-D inductance extraction program*, IEEE Trans. MTT
42(9), 1994); Grover/Rosa closed forms for parallel filament partial
inductances; numerical quadrature for arbitrary orientation. Implemented from
the papers, not from any existing program's source.

## Stack

Rust (stable), `nalgebra` + `simba`/`wide` for portable SIMD, `rayon` for
parallel matrix fill, `num-complex`. No BLAS/LAPACK, no C dependencies, and
`#![forbid(unsafe_code)]`.

## License

MIT — see `LICENSE`. Changes are recorded in `CHANGELOG.md`. Source,
issues, and the contribution rules (including the clean-room rule) live at
<https://github.com/2AMLogic/fasterhenry>.
