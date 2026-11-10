//! Filament currents, current densities and dissipated power.
//!
//! The solves of [`MeshSystem`](crate::MeshSystem) and
//! [`IterativeSystem`](crate::IterativeSystem) already determine every loop
//! current; the port impedance matrix is only the part of that solution seen
//! from the ports. This module exposes the rest: for a port-current
//! excitation `I_p`, the internal loop currents are
//! `I_e = −Z_ee⁻¹ · Z_ep · I_p` (the same system the impedance solves), the
//! loop currents are `[I_e; I_p]`, and the branch (filament) currents are
//! `I_b = Mᵀ · I_m` with the loop basis `M` of [`crate::mesh`]. No physics is
//! added.
//!
//! # Conventions
//!
//! * Currents are **phasor amplitudes** (peak, not RMS), in amperes, for the
//!   excitation as given: the excitation is the phasor of the port currents,
//!   and every current scales linearly with it. Branch `k` is positive from
//!   its segment's node `a` to node `b`.
//! * `|J| = |I| / (w·h)`, in A/m², with `w`, `h` the filament's cross-section.
//! * Power is **time-average** power for peak phasors: `P = ½·|I|²·R`, in
//!   watts, with `R` the filament's DC resistance
//!   ([`filament_resistance`](crate::filament_resistance)). With RMS phasors
//!   the same quantity is `|I|²·R`, i.e. twice the value reported here is
//!   obtained by scaling the excitation by `√2`.
//! * A *conductor* is a connected component of the segment graph (segments
//!   sharing a node); its index is the order of its first segment.
//!
//! # Power balance
//!
//! The power delivered at the ports is `½·Re(I_pᴴ·V_p)`, `V_p = Z_pp·I_p +
//! Z_pe·I_e` (equal to `½·Re(I_pᴴ·Z·I_p)` for the port impedance `Z`). It
//! must equal the sum of the filament dissipations because `L` is real
//! symmetric, so the inductive part of `I_bᴴ(R + jωL)I_b` is purely
//! imaginary. [`PowerBalance::relative_residual`] reports
//! `|P_dissipated − P_input| / max(P_dissipated, P_input)`. For the dense
//! path it is at the level of the LU's rounding (`≲ 1e-9` on the validation
//! fixtures); for the iterative path it is bounded by a small multiple of
//! the GMRES relative-residual tolerance
//! ([`GmresParams::tolerance`](crate::GmresParams)), not by machine epsilon.

use nalgebra::Vector3;
use num_complex::Complex;
use serde::Serialize;

use crate::filament::Filament;
use crate::geometry::Geometry;
use crate::mesh::MeshMatrix;

type C64 = Complex<f64>;

/// Current in one filament.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct FilamentCurrent {
    /// Index of the filament in `filaments()`.
    pub filament: usize,
    /// Segment the filament belongs to.
    pub segment: usize,
    /// Conductor (connected component) the filament belongs to.
    pub conductor: usize,
    /// Centroid of the filament, in metres.
    pub location: [f64; 3],
    /// Complex phasor current (peak amplitude), amperes.
    pub current: C64,
    /// Current density magnitude `|I|/(w·h)`, A/m².
    pub current_density: f64,
    /// Time-average dissipated power `½|I|²R`, watts.
    pub power_w: f64,
}

/// Total current through, and power dissipated in, one segment.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct SegmentCurrent {
    /// Segment index.
    pub segment: usize,
    /// Conductor the segment belongs to.
    pub conductor: usize,
    /// Sum of the segment's filament currents (all share one orientation).
    pub current: C64,
    /// Sum of the segment's filament powers, watts.
    pub power_w: f64,
}

/// The hottest filament of one conductor.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ConductorPeak {
    /// Conductor index.
    pub conductor: usize,
    /// Largest `|J|` among its filaments, A/m².
    pub peak_current_density: f64,
    /// Index of the filament carrying it.
    pub filament: usize,
    /// Centroid of that filament, in metres.
    pub location: [f64; 3],
}

/// Dissipated versus delivered power; see the [module docs](self).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct PowerBalance {
    /// `Σ ½|I|²R` over all filaments, watts.
    pub dissipated_w: f64,
    /// `½ Re(I_pᴴ V_p)`, watts.
    pub input_w: f64,
    /// `|dissipated − input| / max(dissipated, input)`; 0 when both are 0.
    pub relative_residual: f64,
}

/// All currents of one frequency and excitation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CurrentSolution {
    /// Frequency, hertz.
    pub frequency_hz: f64,
    /// The port-current phasors that were applied, amperes.
    pub excitation: Vec<C64>,
    /// Every filament, in `filaments()` order.
    pub filaments: Vec<FilamentCurrent>,
    /// Every segment.
    pub segments: Vec<SegmentCurrent>,
    /// One entry per conductor, in conductor order.
    pub conductors: Vec<ConductorPeak>,
    /// Dissipated versus delivered power.
    pub power_balance: PowerBalance,
}

/// The default excitation: 1 A into port 1, every other port open (0 A).
pub fn unit_excitation(ports: usize) -> Vec<C64> {
    let mut excitation = vec![C64::new(0.0, 0.0); ports];
    if let Some(first) = excitation.first_mut() {
        *first = C64::new(1.0, 0.0);
    }
    excitation
}

/// Segment/conductor bookkeeping kept by a system so currents can be
/// attributed after the solve.
#[derive(Clone, Debug)]
pub(crate) struct Layout {
    /// Segment of every filament.
    pub(crate) segment_of_filament: Vec<usize>,
    /// Conductor of every segment.
    pub(crate) conductor_of_segment: Vec<usize>,
    pub(crate) conductor_count: usize,
}

impl Layout {
    pub(crate) fn new(geometry: &Geometry, per_segment: &[usize]) -> Self {
        let defs = geometry.segment_defs();
        let mut parent: Vec<usize> = (0..geometry.nodes().len()).collect();
        fn find(parent: &mut [usize], mut x: usize) -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]];
                x = parent[x];
            }
            x
        }
        for def in defs {
            let (a, b) = (find(&mut parent, def.a.0), find(&mut parent, def.b.0));
            if a != b {
                parent[a] = b;
            }
        }
        let mut roots: Vec<usize> = Vec::new();
        let conductor_of_segment: Vec<usize> = defs
            .iter()
            .map(|def| {
                let root = find(&mut parent, def.a.0);
                roots.iter().position(|&r| r == root).unwrap_or_else(|| {
                    roots.push(root);
                    roots.len() - 1
                })
            })
            .collect();
        let segment_of_filament = per_segment
            .iter()
            .enumerate()
            .flat_map(|(segment, &n)| std::iter::repeat_n(segment, n))
            .collect();
        Self {
            segment_of_filament,
            conductor_of_segment,
            conductor_count: roots.len(),
        }
    }
}

/// Turns solved loop currents into the full [`CurrentSolution`].
///
/// `internal` are the internal loop currents, `port_voltage` the port
/// voltages `V_p` of the same solution.
#[allow(clippy::too_many_arguments)]
pub(crate) fn assemble_solution(
    layout: &Layout,
    filaments: &[Filament],
    resistances: &[f64],
    mesh: &MeshMatrix,
    frequency_hz: f64,
    excitation: &[C64],
    internal: &[C64],
    port_voltage: &[C64],
) -> CurrentSolution {
    let mut branch = vec![C64::new(0.0, 0.0); filaments.len()];
    let loops = internal.iter().chain(excitation.iter());
    for (i, &x) in loops.enumerate() {
        for &(b, sign) in mesh.row(i) {
            branch[b] += x * sign;
        }
    }

    let mut out = Vec::with_capacity(filaments.len());
    let mut segments: Vec<SegmentCurrent> = layout
        .conductor_of_segment
        .iter()
        .enumerate()
        .map(|(segment, &conductor)| SegmentCurrent {
            segment,
            conductor,
            current: C64::new(0.0, 0.0),
            power_w: 0.0,
        })
        .collect();
    let mut conductors: Vec<Option<ConductorPeak>> = vec![None; layout.conductor_count];
    let mut dissipated = 0.0;
    for (k, ((f, &i), &r)) in filaments.iter().zip(&branch).zip(resistances).enumerate() {
        let segment = layout.segment_of_filament[k];
        let conductor = layout.conductor_of_segment[segment];
        let center: Vector3<f64> = f.center();
        let location = [center.x, center.y, center.z];
        let density = i.norm() / (f.width() * f.height());
        let power = 0.5 * i.norm_sqr() * r;
        dissipated += power;
        segments[segment].current += i;
        segments[segment].power_w += power;
        let peak = &mut conductors[conductor];
        if peak.is_none_or(|p| density > p.peak_current_density) {
            *peak = Some(ConductorPeak {
                conductor,
                peak_current_density: density,
                filament: k,
                location,
            });
        }
        out.push(FilamentCurrent {
            filament: k,
            segment,
            conductor,
            location,
            current: i,
            current_density: density,
            power_w: power,
        });
    }
    let input: f64 = 0.5
        * excitation
            .iter()
            .zip(port_voltage)
            .map(|(i, v)| (i.conj() * v).re)
            .sum::<f64>();
    let scale = dissipated.max(input);
    CurrentSolution {
        frequency_hz,
        excitation: excitation.to_vec(),
        filaments: out,
        segments,
        conductors: conductors.into_iter().flatten().collect(),
        power_balance: PowerBalance {
            dissipated_w: dissipated,
            input_w: input,
            relative_residual: if scale > 0.0 {
                (dissipated - input).abs() / scale
            } else {
                0.0
            },
        },
    }
}
