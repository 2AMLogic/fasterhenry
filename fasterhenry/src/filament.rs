//! Filaments — the PEEC current elements — and the uniform discretization of
//! a [`Segment`] into an `nw × nh` bundle of them.
//!
//! Following Kamon, Tsuk & White (IEEE T-MTT 1994, §II), the current in a
//! segment is approximated as piecewise constant over its cross-section: the
//! segment is cut into parallel filaments, each a thinner rectangular bar
//! spanning the segment's full length and carrying a uniform current density
//! along it. More filaments resolve skin and proximity effects better. This
//! module provides the uniform grid; the skin-effect-aware non-uniform grid is
//! later work.

use nalgebra::Vector3;
use serde::Serialize;
use thiserror::Error;

use crate::geometry::{LocalBasis, Segment, SegmentError};

/// A straight current filament of rectangular cross-section.
///
/// A filament always carries a consistent orthonormal [`LocalBasis`], so it is
/// only created through [`Filament::new`] or [`discretize`] and exposes its
/// data through accessors.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Filament {
    start: Vector3<f64>,
    end: Vector3<f64>,
    length: f64,
    basis: LocalBasis,
    width: f64,
    height: f64,
    sigma: f64,
}

impl Filament {
    /// Creates a single filament occupying the whole cross-section of
    /// `segment`; equivalent to `discretize(segment, 1, 1)`.
    pub fn new(segment: &Segment) -> Result<Self, SegmentError> {
        let basis = segment.basis()?;
        Ok(Self::from_parts(
            segment.a.position(),
            segment.b.position(),
            segment.length(),
            basis,
            segment.width,
            segment.height,
            segment.sigma,
        ))
    }

    fn from_parts(
        start: Vector3<f64>,
        end: Vector3<f64>,
        length: f64,
        basis: LocalBasis,
        width: f64,
        height: f64,
        sigma: f64,
    ) -> Self {
        Self {
            start,
            end,
            length,
            basis,
            width,
            height,
            sigma,
        }
    }

    /// Start of the centreline (the end nearer the segment's node `a`).
    pub fn start(&self) -> Vector3<f64> {
        self.start
    }

    /// End of the centreline (the end nearer the segment's node `b`).
    pub fn end(&self) -> Vector3<f64> {
        self.end
    }

    /// Midpoint of the centreline — the filament's centroid.
    pub fn center(&self) -> Vector3<f64> {
        (self.start + self.end) * 0.5
    }

    /// Centreline length.
    pub fn length(&self) -> f64 {
        self.length
    }

    /// The filament's right-handed orthonormal frame.
    pub fn basis(&self) -> &LocalBasis {
        &self.basis
    }

    /// Unit vector along the direction of current flow, from
    /// [`start`](Self::start) to [`end`](Self::end).
    pub fn direction(&self) -> Vector3<f64> {
        self.basis.length
    }

    /// Unit vector along the cross-section's width.
    pub fn width_dir(&self) -> Vector3<f64> {
        self.basis.width
    }

    /// Unit vector along the cross-section's height.
    pub fn height_dir(&self) -> Vector3<f64> {
        self.basis.height
    }

    /// Cross-section extent along [`width_dir`](Self::width_dir).
    pub fn width(&self) -> f64 {
        self.width
    }

    /// Cross-section extent along [`height_dir`](Self::height_dir).
    pub fn height(&self) -> f64 {
        self.height
    }

    /// Cross-section area, `width · height`.
    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    /// Electrical conductivity, inherited from the segment.
    pub fn sigma(&self) -> f64 {
        self.sigma
    }
}

/// Why [`discretize`] failed.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum DiscretizeError {
    /// The segment itself is not a valid conductor.
    #[error(transparent)]
    Segment(#[from] SegmentError),
    /// A filament count of zero was requested along the width or the height.
    #[error("filament counts must be at least 1, got nw = {nw}, nh = {nh}")]
    ZeroSubdivision {
        /// Requested number of filaments across the width.
        nw: usize,
        /// Requested number of filaments across the height.
        nh: usize,
    },
}

/// Splits `segment` into `nw × nh` parallel filaments on a uniform grid over
/// its cross-section: `nw` across the width, `nh` across the height.
///
/// Every filament spans the segment's full length, shares its
/// [`LocalBasis`] and conductivity, and has cross-section
/// `(width / nw) × (height / nh)`, so the filaments tile the segment exactly.
/// The centreline of filament `(i, j)` — `i` indexing width, `j` height — is
/// the segment's centreline shifted by
///
/// ```text
/// ((i + ½)/nw − ½) · width · ŵ  +  ((j + ½)/nh − ½) · height · ĥ
/// ```
///
/// The result is ordered with the width index fastest: filament `(i, j)` is
/// at position `j · nw + i`.
///
/// # Errors
///
/// [`DiscretizeError::ZeroSubdivision`] if `nw` or `nh` is zero, and
/// [`DiscretizeError::Segment`] if `segment` fails [`Segment::validate`].
///
/// # Example
///
/// ```
/// use fasterhenry::{discretize, Node, Segment};
///
/// // A 10 mm copper trace, 1 mm wide and 35 µm thick, along +x.
/// let trace = Segment::new(
///     Node::new(0.0, 0.0, 0.0),
///     Node::new(10e-3, 0.0, 0.0),
///     1e-3,
///     35e-6,
///     5.8e7,
/// );
/// let filaments = discretize(&trace, 5, 2)?;
/// assert_eq!(filaments.len(), 10);
///
/// let total_area: f64 = filaments.iter().map(|f| f.area()).sum();
/// assert!((total_area - trace.area()).abs() < 1e-12 * trace.area());
/// # Ok::<(), fasterhenry::DiscretizeError>(())
/// ```
pub fn discretize(
    segment: &Segment,
    nw: usize,
    nh: usize,
) -> Result<Vec<Filament>, DiscretizeError> {
    if nw == 0 || nh == 0 {
        return Err(DiscretizeError::ZeroSubdivision { nw, nh });
    }
    let basis = segment.basis()?;
    let (a, b) = (segment.a.position(), segment.b.position());
    // Shared by the whole bundle, so parallel filaments compare equal in length.
    let length = segment.length();
    let filament_width = segment.width / nw as f64;
    let filament_height = segment.height / nh as f64;

    // Offset of cell `index` of `count` from the centreline, as a fraction of
    // the full extent: cell centres of a uniform grid on [−½, ½].
    let fraction = |index: usize, count: usize| (index as f64 + 0.5) / count as f64 - 0.5;

    let mut filaments = Vec::with_capacity(nw * nh);
    for j in 0..nh {
        let along_height = basis.height * (fraction(j, nh) * segment.height);
        for i in 0..nw {
            let offset = basis.width * (fraction(i, nw) * segment.width) + along_height;
            filaments.push(Filament::from_parts(
                a + offset,
                b + offset,
                length,
                basis,
                filament_width,
                filament_height,
                segment.sigma,
            ));
        }
    }
    Ok(filaments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Node;
    use nalgebra::{Rotation3, Unit};

    const COPPER: f64 = 5.8e7;
    const TOL: f64 = 1e-12;

    fn assert_vec_close(actual: Vector3<f64>, expected: [f64; 3]) {
        let expected = Vector3::from(expected);
        assert!(
            (actual - expected).norm() < TOL,
            "expected {expected:?}, got {actual:?}"
        );
    }

    /// A segment in general position, with an explicit width direction.
    fn skew_segment() -> Segment {
        Segment::new(
            Node::new(0.3, -1.2, 2.0),
            Node::new(1.7, 0.4, -0.5),
            0.6,
            0.25,
            COPPER,
        )
        .with_width_dir([0.2, 1.0, 0.1])
    }

    #[test]
    fn filament_count_is_nw_times_nh() {
        for (nw, nh) in [(1, 1), (1, 4), (3, 1), (3, 2), (5, 7)] {
            let filaments = discretize(&skew_segment(), nw, nh).unwrap();
            assert_eq!(filaments.len(), nw * nh, "nw = {nw}, nh = {nh}");
        }
    }

    #[test]
    fn mean_of_filament_centroids_is_the_segment_centreline() {
        let segment = skew_segment();
        for (nw, nh) in [(1, 1), (2, 2), (3, 2), (5, 7)] {
            let filaments = discretize(&segment, nw, nh).unwrap();
            let n = filaments.len() as f64;
            let mean = |f: fn(&Filament) -> Vector3<f64>| {
                filaments.iter().map(f).sum::<Vector3<f64>>() / n
            };
            // The whole centreline matches, not just its midpoint.
            assert!((mean(Filament::start) - segment.a.position()).norm() < TOL);
            assert!((mean(Filament::end) - segment.b.position()).norm() < TOL);
            assert!((mean(Filament::center) - segment.center()).norm() < TOL);
        }
    }

    #[test]
    fn total_cross_section_area_is_preserved() {
        let segment = skew_segment();
        for (nw, nh) in [(1, 1), (2, 2), (3, 2), (5, 7)] {
            let filaments = discretize(&segment, nw, nh).unwrap();
            let total: f64 = filaments.iter().map(Filament::area).sum();
            assert!(
                (total - segment.area()).abs() < TOL * segment.area(),
                "nw = {nw}, nh = {nh}: {total} vs {}",
                segment.area()
            );
            for f in &filaments {
                assert!((f.width() - segment.width / nw as f64).abs() < TOL);
                assert!((f.height() - segment.height / nh as f64).abs() < TOL);
            }
        }
    }

    #[test]
    fn filaments_inherit_length_basis_and_conductivity() {
        let segment = skew_segment();
        let basis = segment.basis().unwrap();
        for f in discretize(&segment, 3, 2).unwrap() {
            assert!((f.length() - segment.length()).abs() < TOL);
            assert!(((f.end() - f.start()) / f.length() - f.direction()).norm() < TOL);
            assert_eq!(f.basis(), &basis);
            assert_eq!(f.direction(), basis.length);
            assert_eq!(f.width_dir(), basis.width);
            assert_eq!(f.height_dir(), basis.height);
            assert_eq!(f.sigma(), COPPER);
        }
    }

    #[test]
    fn filaments_tile_the_cross_section_in_documented_order() {
        // Axis-aligned, so the grid can be read off directly: width along +y,
        // height along +z.
        let segment = Segment::new(
            Node::new(0.0, 0.0, 0.0),
            Node::new(2.0, 0.0, 0.0),
            0.6,
            0.4,
            COPPER,
        );
        let filaments = discretize(&segment, 3, 2).unwrap();
        let expected_yz = [
            [-0.2, -0.1],
            [0.0, -0.1],
            [0.2, -0.1],
            [-0.2, 0.1],
            [0.0, 0.1],
            [0.2, 0.1],
        ];
        assert_eq!(filaments.len(), expected_yz.len());
        for (f, [y, z]) in filaments.iter().zip(expected_yz) {
            assert_vec_close(f.start(), [0.0, y, z]);
            assert_vec_close(f.end(), [2.0, y, z]);
        }
    }

    #[test]
    fn single_filament_is_the_segment_itself() {
        let segment = skew_segment();
        let filaments = discretize(&segment, 1, 1).unwrap();
        assert_eq!(filaments, vec![Filament::new(&segment).unwrap()]);
        assert_vec_close(filaments[0].start(), [0.3, -1.2, 2.0]);
        assert_vec_close(filaments[0].end(), [1.7, 0.4, -0.5]);
        assert!((filaments[0].area() - 0.15).abs() < TOL);
    }

    /// Hand-rotated fixture 1: a quarter turn about z, default orientation.
    ///
    /// Base segment: (0,0,0)→(2,0,0), width 0.4 along +y, split in two, so its
    /// filaments sit at y = −0.1 and y = +0.1. Rotating +90° about z maps
    /// (x, y, z) → (−y, x, z): the segment becomes (0,0,0)→(0,2,0) and the
    /// filaments move to x = +0.1 and x = −0.1, in that order.
    #[test]
    fn quarter_turn_about_z_matches_hand_rotated_fixture() {
        let rotated = Segment::new(
            Node::new(0.0, 0.0, 0.0),
            Node::new(0.0, 2.0, 0.0),
            0.4,
            0.2,
            COPPER,
        );
        let filaments = discretize(&rotated, 2, 1).unwrap();
        assert_eq!(filaments.len(), 2);

        assert_vec_close(filaments[0].start(), [0.1, 0.0, 0.0]);
        assert_vec_close(filaments[0].end(), [0.1, 2.0, 0.0]);
        assert_vec_close(filaments[1].start(), [-0.1, 0.0, 0.0]);
        assert_vec_close(filaments[1].end(), [-0.1, 2.0, 0.0]);
        for f in &filaments {
            assert_vec_close(f.direction(), [0.0, 1.0, 0.0]);
            assert_vec_close(f.width_dir(), [-1.0, 0.0, 0.0]);
            assert_vec_close(f.height_dir(), [0.0, 0.0, 1.0]);
            assert!((f.width() - 0.2).abs() < TOL);
            assert!((f.height() - 0.2).abs() < TOL);
        }
    }

    /// Hand-rotated fixture 2: an in-plane 45° segment, default orientation.
    ///
    /// Base segment: (0,0,0)→(√2,0,0), width 0.4·√2 along +y, split in two:
    /// filaments at y = ∓0.1·√2. Rotating +45° about z sends x̂ → (1,1,0)/√2
    /// and ŷ → (−1,1,0)/√2, so the segment becomes (0,0,0)→(1,1,0) and the
    /// filament offsets become ∓0.1·√2·(−1,1,0)/√2 = ±(0.1, −0.1, 0).
    #[test]
    fn diagonal_segment_matches_hand_rotated_fixture() {
        let rotated = Segment::new(
            Node::new(0.0, 0.0, 0.0),
            Node::new(1.0, 1.0, 0.0),
            0.4 * std::f64::consts::SQRT_2,
            0.2,
            COPPER,
        );
        let filaments = discretize(&rotated, 2, 1).unwrap();
        assert_eq!(filaments.len(), 2);

        assert_vec_close(filaments[0].start(), [0.1, -0.1, 0.0]);
        assert_vec_close(filaments[0].end(), [1.1, 0.9, 0.0]);
        assert_vec_close(filaments[1].start(), [-0.1, 0.1, 0.0]);
        assert_vec_close(filaments[1].end(), [0.9, 1.1, 0.0]);
        let s = std::f64::consts::FRAC_1_SQRT_2;
        for f in &filaments {
            assert_vec_close(f.direction(), [s, s, 0.0]);
            assert_vec_close(f.width_dir(), [-s, s, 0.0]);
            assert_vec_close(f.height_dir(), [0.0, 0.0, 1.0]);
        }
    }

    /// Hand-rotated fixture 3: an out-of-plane rotation with an explicit
    /// width direction.
    ///
    /// Base segment: (1,2,3)→(4,2,3), width 0.6 along +y, height 0.3 along
    /// +z, on a 3 × 2 grid: width offsets −0.2, 0, +0.2 and height offsets
    /// ∓0.075. A 120° turn about (1,1,1) permutes the axes cyclically,
    /// (x, y, z) → (z, x, y): the segment becomes (3,1,2)→(3,4,2), its width
    /// direction ŷ → ẑ and its height direction ẑ → x̂.
    #[test]
    fn cyclic_axis_rotation_matches_hand_rotated_fixture() {
        let rotated = Segment::new(
            Node::new(3.0, 1.0, 2.0),
            Node::new(3.0, 4.0, 2.0),
            0.6,
            0.3,
            COPPER,
        )
        .with_width_dir([0.0, 0.0, 1.0]);
        let filaments = discretize(&rotated, 3, 2).unwrap();

        // (x, z) of each centreline; y runs 1 → 4 for all of them.
        let expected_xz = [
            [2.925, 1.8],
            [2.925, 2.0],
            [2.925, 2.2],
            [3.075, 1.8],
            [3.075, 2.0],
            [3.075, 2.2],
        ];
        assert_eq!(filaments.len(), expected_xz.len());
        for (f, [x, z]) in filaments.iter().zip(expected_xz) {
            assert_vec_close(f.start(), [x, 1.0, z]);
            assert_vec_close(f.end(), [x, 4.0, z]);
            assert_vec_close(f.direction(), [0.0, 1.0, 0.0]);
            assert_vec_close(f.width_dir(), [0.0, 0.0, 1.0]);
            assert_vec_close(f.height_dir(), [1.0, 0.0, 0.0]);
            assert!((f.width() - 0.2).abs() < TOL);
            assert!((f.height() - 0.15).abs() < TOL);
        }
    }

    /// Discretizing commutes with rigid motion: rotating and translating a
    /// segment (and its width direction) rotates and translates its filaments.
    #[test]
    fn discretization_commutes_with_arbitrary_rigid_motion() {
        let base = skew_segment();
        let rotation =
            Rotation3::from_axis_angle(&Unit::new_normalize(Vector3::new(0.4, -1.0, 0.7)), 1.234);
        let shift = Vector3::new(-2.0, 0.5, 3.0);
        let moved = Segment {
            a: Node::from(rotation * base.a.position() + shift),
            b: Node::from(rotation * base.b.position() + shift),
            width_dir: base
                .width_dir
                .map(|dir| (rotation * Vector3::from(dir)).into()),
            ..base
        };

        let before = discretize(&base, 4, 3).unwrap();
        let after = discretize(&moved, 4, 3).unwrap();
        assert_eq!(after.len(), 12);
        for (f, g) in before.iter().zip(&after) {
            assert!((rotation * f.start() + shift - g.start()).norm() < TOL);
            assert!((rotation * f.end() + shift - g.end()).norm() < TOL);
            assert!((rotation * f.direction() - g.direction()).norm() < TOL);
            assert!((rotation * f.width_dir() - g.width_dir()).norm() < TOL);
            assert!((rotation * f.height_dir() - g.height_dir()).norm() < TOL);
            assert!((f.width() - g.width()).abs() < TOL);
            assert!((f.height() - g.height()).abs() < TOL);
        }
    }

    #[test]
    fn zero_subdivision_is_rejected() {
        for (nw, nh) in [(0, 1), (1, 0), (0, 0)] {
            assert_eq!(
                discretize(&skew_segment(), nw, nh),
                Err(DiscretizeError::ZeroSubdivision { nw, nh })
            );
        }
    }

    #[test]
    fn invalid_segment_is_rejected() {
        let segment = skew_segment();
        let zero_length = Segment {
            b: segment.a,
            ..segment
        };
        assert_eq!(
            discretize(&zero_length, 2, 2),
            Err(DiscretizeError::Segment(SegmentError::ZeroLength))
        );
        assert_eq!(Filament::new(&zero_length), Err(SegmentError::ZeroLength));
        let flat = Segment {
            height: 0.0,
            ..segment
        };
        assert!(matches!(
            discretize(&flat, 2, 2),
            Err(DiscretizeError::Segment(
                SegmentError::NonPositiveDimension {
                    dimension: "height",
                    ..
                }
            ))
        ));
    }

    #[test]
    fn filament_serializes_to_json() {
        let segment = Segment::new(
            Node::new(0.0, 0.0, 0.0),
            Node::new(2.0, 0.0, 0.0),
            0.6,
            0.4,
            COPPER,
        );
        let value = serde_json::to_value(Filament::new(&segment).unwrap()).unwrap();
        assert_eq!(value["start"], serde_json::json!([0.0, 0.0, 0.0]));
        assert_eq!(value["end"], serde_json::json!([2.0, 0.0, 0.0]));
        assert_eq!(value["basis"]["width"], serde_json::json!([0.0, 1.0, 0.0]));
        assert_eq!(value["length"], serde_json::json!(2.0));
    }
}
