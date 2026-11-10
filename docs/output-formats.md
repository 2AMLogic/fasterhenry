# Output formats

How a `fasterhenry` run's results reach other tools. The JSON form (always
emitted, to stdout or `--json`) is the primary, versioned one
(`schema_version`, provenance, exact `f64` round-trips); the formats below
exist for tool interop.

## `--zc-mat out.mat` — MATLAB level-4 binary

(Also written as `./Zc.mat`, without the flag, by the drop-in invocation
`fasterhenry <deck>` — see `fasterhenry-cli/README.md` § "Migrating from
FastHenry".)

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
within `1e-12` of the bound are clamped.

The subcircuit name (used identically on `.subckt` and `.ends`) comes
from the input file's stem, normalized to one conservative ASCII
identifier token: every character outside `[A-Za-z0-9_]` — whitespace,
tabs, newlines, punctuation including `.`, and any non-ASCII character —
becomes one `_`, so `my coil.inp` exports `my_coil` and `v1.2.inp`
exports `v1_2`, while a simple stem such as `spiral.inp` exports
`spiral` unchanged. A path with no usable stem falls back to
`fasterhenry`. Filename text therefore cannot add tokens or netlist lines.

**Limitations**: linear-small-signal at the chosen frequency only (the
stamp is exact for AC analysis at `--spice-freq` by construction); a
frequency-dependent rerun is a re-export.

## `--currents out.csv` — filament currents

Exposes the loop/filament currents the solve already determines (no new
physics) for **1 A into port 1, every other port open**, at every frequency of
the sweep. Library equivalents: `MeshSystem::currents` and
`IterativeSystem::currents` (`fasterhenry::currents`), which take any
port-current vector.

**CSV**, streamed (one frequency's rows are in memory at a time), header then
one row per filament per frequency:

| column | unit | meaning |
|---|---|---|
| `frequency_hz` | Hz | sweep frequency |
| `filament`, `segment`, `conductor` | – | indices; a conductor is a connected component of the segment graph |
| `x_m`, `y_m`, `z_m` | m | filament centroid |
| `current_re_a`, `current_im_a`, `current_abs_a` | A | complex phasor current, positive from the segment's `a` node to `b` |
| `j_abs_a_per_m2` | A/m² | `\|I\|/(w·h)` of the filament |
| `power_w` | W | `½·\|I\|²·R`, `R` = the filament's DC resistance |

**Convention.** Currents are *peak* phasor amplitudes for the stated excitation;
power is time-average power for peak phasors, `½\|I\|²R` (with RMS phasors it
would be `\|I\|²R`).

**JSON.** With the flag, the result gains a top-level `currents` object:
`by_frequency[k]` holds `frequency_hz`, `power_balance`
(`dissipated_w = Σ½\|I\|²R`, `input_w = ½Re(I_pᴴV_p)`, `relative_residual`),
`segments` (summed current and power per segment) and `conductors` (peak `\|J\|`
and its filament and location). The block is additive: without the flag the
JSON, `Zc.mat` and SPICE outputs are unchanged and `schema_version` stays 1.

**Power balance tolerance.** Dense path: `relative_residual ≲ 1e-9` on the
validation fixtures. Iterative path: bounded by a small multiple of the GMRES
relative-residual tolerance (default `1e-10`), not by machine epsilon.

## Validation of both formats

`tests/roundtrip.rs` checks the MAT byte layout structurally; CI loads a
generated file with `scipy.io.loadmat` and compares against the JSON
output; the SPICE stamp is exercised end-to-end in CI (`tools/spice_check.py`):
the exported subcircuit is driven in ngspice with a 1 A AC source on each port
in turn, for a one-port deck (`spiral.inp`) and a two-port deck (`twobar.inp`,
which emits the `K` mutual and the off-diagonal `H` sources), and every `V(p_i)`
is compared with the JSON `Z_ij(1 GHz)` in both parts to a relative tolerance
of 1e-6.
