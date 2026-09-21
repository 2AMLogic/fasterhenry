# Output formats

How `fasterhenry run`'s results reach other tools. The JSON form (always
emitted, to stdout or `--json`) is the primary, versioned one
(`schema_version`, provenance, exact `f64` round-trips); the formats below
exist for tool interop.

## `--zc-mat out.mat` — MATLAB level-4 binary

The format of FastHenry's `Zc.mat`, written from the published on-disk
specification. A MAT v4 file is a sequence of matrices; `fasterhenry`
writes:

| variable | shape | meaning |
|---|---|---|
| `freqs` | 1 × K | the sweep, hertz |
| `Zc_1 … Zc_K` | n_ports × n_ports, complex | the impedance matrix at each frequency, ohms |

Each record is a 20-byte header — `mopt` (int32, `0`: native byte order,
IEEE double, full numeric matrix), `m`, `n`, `imagf` (int32), `namlen`
(int32) — followed by the exact name bytes and, column-major, all real
parts then all imaginary parts. Verified readers: `scipy.io.loadmat`
(round-trips exactly, asserted in CI) and Octave's `-mat4-binary`
(same layout).

MAT v4 has no 3-D matrices, hence one `Zc_k` per frequency rather than a
page stack; a small loader loop is the whole difference.

## `--spice out.cir [--spice-freq HZ]` — SPICE subcircuit

A lumped stamp of `Z = R + jωL` at one frequency (default: the sweep's
last). For each port `i`, with external nodes `p<i>`/`n<i>` (current into
`p<i>` is positive):

```
p<i> — L<i> — H_i_1 — H_i_2 — … — vsense<i> — n<i>
```

* `L<i> = L_ii` carries the self inductance; `K_i_j` elements couple the
  port inductors with `K_ij = L_ij / √(L_ii L_jj)`, reproducing the
  inductive off-diagonal terms.
* one `H_i_j` per matrix entry adds `R_ij · I_j` to port `i`'s voltage —
  the resistive part, which coupled inductors cannot express. `vsense<j>`
  is a 0 V source sensing port `j`'s current.
* entries with `R_ij = 0` emit no source; at DC (`f = 0`) the inductors
  are zero and the `K` lines are omitted — a purely resistive stamp.

Because the underlying partial-inductance matrix is positive
semidefinite, `|K_ij| ≤ 1` holds mathematically; floating-point results
within `1e-12` of the bound are clamped. Subcircuit and file names come
from the input file's stem.

**Limitations**: linear-small-signal at the chosen frequency only (the
stamp is exact for AC analysis at `--spice-freq` by construction); a
frequency-dependent rerun is a re-export.

## Validation of both formats

`tests/roundtrip.rs` checks the MAT byte layout structurally; CI loads a
generated file with `scipy.io.loadmat` and compares against the JSON
output; the SPICE stamp is exercised end-to-end by driving the exported
spiral subcircuit with a 1 A AC source in ngspice and comparing `V(p1)`
against `Z(1 GHz)` (both parts to six digits in the development run).
