//! Conductor geometry: nodes, straight rectangular-cross-section segments,
//! and the validated [`Geometry`] container every later stage consumes.
//!
//! The model follows the PEEC discretization of Ruehli and of Kamon, Tsuk &
//! White (IEEE T-MTT 1994, §II): a conductor is a chain of straight segments,
//! each a rectangular bar of uniform conductivity running between two nodes.
//! Segments that share a node are electrically connected there.
//!
//! # Units
//!
//! The library is unit-agnostic but assumes a *consistent* system. SI is
//! recommended: coordinates and cross-section dimensions in metres,
//! conductivity in siemens per metre.
//!
//! # Cross-section orientation
//!
//! A segment's direction of current flow (its *length* direction) is fixed by
//! its two nodes. The rotation of the rectangular cross-section about that
//! axis is fixed by the *width direction*, see [`Segment::width_dir`] and
//! [`Segment::basis`].

use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Below this value of `|ẑ × l̂|` (the sine of the angle between a segment and
/// the z axis) a segment counts as vertical when choosing the default width
/// direction.
const VERTICAL_SIN_TOLERANCE: f64 = 1.0e-6;

/// An explicit width direction whose component perpendicular to the segment is
/// smaller than this fraction of its norm counts as parallel to the segment.
const PARALLEL_TOLERANCE: f64 = 1.0e-9;

/// A point in 3-D space at which segments begin, end, and connect.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// x coordinate.
    pub x: f64,
    /// y coordinate.
    pub y: f64,
    /// z coordinate.
    pub z: f64,
}

impl Node {
    /// Creates a node at `(x, y, z)`.
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// The node's position as a vector from the origin.
    pub fn position(&self) -> Vector3<f64> {
        Vector3::new(self.x, self.y, self.z)
    }

    /// `true` when all three coordinates are finite (neither NaN nor ±∞).
    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl From<[f64; 3]> for Node {
    fn from([x, y, z]: [f64; 3]) -> Self {
        Self { x, y, z }
    }
}

impl From<Vector3<f64>> for Node {
    fn from(v: Vector3<f64>) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

/// A right-handed orthonormal frame attached to a segment or filament.
///
/// `width × height = length`, so `(width, height, length)` is a right-handed
/// triad: `length` is the direction of current flow, `width` and `height`
/// span the rectangular cross-section.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalBasis {
    /// Unit vector along the direction of current flow (from `a` to `b`).
    pub length: Vector3<f64>,
    /// Unit vector along the cross-section's width.
    pub width: Vector3<f64>,
    /// Unit vector along the cross-section's height.
    pub height: Vector3<f64>,
}

/// Why a single [`Segment`] is not a valid conductor.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum SegmentError {
    /// An end-point coordinate is NaN or infinite.
    #[error("segment end point has a non-finite coordinate")]
    NonFiniteEndpoint,
    /// The two end points coincide, so the segment has no direction.
    #[error("segment has zero length (its end points coincide)")]
    ZeroLength,
    /// The width or height is zero, negative, NaN, or infinite.
    #[error("segment {dimension} must be positive and finite, got {value}")]
    NonPositiveDimension {
        /// Which dimension is invalid: `"width"` or `"height"`.
        dimension: &'static str,
        /// The offending value.
        value: f64,
    },
    /// The conductivity is zero, negative, NaN, or infinite.
    #[error("segment conductivity must be positive and finite, got {value}")]
    NonPositiveConductivity {
        /// The offending value.
        value: f64,
    },
    /// The explicit width direction is zero, non-finite, or parallel to the
    /// segment, so it does not fix the cross-section's orientation.
    #[error("segment width direction is zero, non-finite, or parallel to the segment")]
    InvalidWidthDirection,
}

/// A straight conductor of rectangular cross-section between two nodes.
///
/// Current flows along the centreline from [`a`](Self::a) to [`b`](Self::b).
/// The fields are public so a segment can be written as a literal; call
/// [`validate`](Self::validate) (or go through [`Geometry`], which does)
/// before relying on it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    /// Start of the centreline.
    pub a: Node,
    /// End of the centreline.
    pub b: Node,
    /// Cross-section extent along the width direction.
    pub width: f64,
    /// Cross-section extent along the height direction.
    pub height: f64,
    /// Electrical conductivity.
    pub sigma: f64,
    /// Optional explicit width direction, as `[x, y, z]`.
    ///
    /// It need not be normalized nor exactly perpendicular to the segment:
    /// [`basis`](Self::basis) uses its component perpendicular to the
    /// centreline. When `None`, the default rule documented on
    /// [`basis`](Self::basis) applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_dir: Option<[f64; 3]>,
}

impl Segment {
    /// Creates a segment with the default cross-section orientation.
    pub const fn new(a: Node, b: Node, width: f64, height: f64, sigma: f64) -> Self {
        Self {
            a,
            b,
            width,
            height,
            sigma,
            width_dir: None,
        }
    }

    /// Returns the segment with an explicit width direction.
    #[must_use]
    pub const fn with_width_dir(mut self, width_dir: [f64; 3]) -> Self {
        self.width_dir = Some(width_dir);
        self
    }

    /// Centreline length, `|b − a|`.
    pub fn length(&self) -> f64 {
        (self.b.position() - self.a.position()).norm()
    }

    /// Midpoint of the centreline.
    pub fn center(&self) -> Vector3<f64> {
        (self.a.position() + self.b.position()) * 0.5
    }

    /// Cross-section area, `width · height`.
    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    /// Checks that the segment is a well-formed conductor: finite end points,
    /// non-zero length, positive finite width, height and conductivity, and a
    /// usable width direction.
    pub fn validate(&self) -> Result<(), SegmentError> {
        self.basis().map(|_| ())
    }

    /// The segment's local right-handed frame, validating the segment first.
    ///
    /// * `length` is the unit vector from `a` to `b`.
    /// * `width` is, when [`width_dir`](Self::width_dir) is given, the
    ///   normalized component of that vector perpendicular to `length`.
    ///   Otherwise it is `ẑ × length` normalized, which lies in the x–y plane
    ///   — so a segment along `+x` has its width along `+y` and its height
    ///   along `+z`. A segment parallel to the z axis (for which that product
    ///   vanishes) takes `+x` as its width direction.
    /// * `height = length × width`.
    pub fn basis(&self) -> Result<LocalBasis, SegmentError> {
        if !(self.a.is_finite() && self.b.is_finite()) {
            return Err(SegmentError::NonFiniteEndpoint);
        }
        for (dimension, value) in [("width", self.width), ("height", self.height)] {
            if !(value.is_finite() && value > 0.0) {
                return Err(SegmentError::NonPositiveDimension { dimension, value });
            }
        }
        if !(self.sigma.is_finite() && self.sigma > 0.0) {
            return Err(SegmentError::NonPositiveConductivity { value: self.sigma });
        }

        let span = self.b.position() - self.a.position();
        let norm = span.norm();
        if !(norm.is_finite() && norm > 0.0) {
            return Err(SegmentError::ZeroLength);
        }
        let length = span / norm;

        let hint = match self.width_dir {
            Some(dir) => {
                let dir = Vector3::from(dir);
                let dir_norm = dir.norm();
                if !(dir_norm.is_finite() && dir_norm > 0.0) {
                    return Err(SegmentError::InvalidWidthDirection);
                }
                let dir = dir / dir_norm;
                let perpendicular = dir - length * dir.dot(&length);
                if perpendicular.norm() <= PARALLEL_TOLERANCE {
                    return Err(SegmentError::InvalidWidthDirection);
                }
                perpendicular
            }
            None => {
                let in_plane = Vector3::z().cross(&length);
                if in_plane.norm() < VERTICAL_SIN_TOLERANCE {
                    Vector3::x()
                } else {
                    in_plane
                }
            }
        };

        // Gram–Schmidt against `length`, so the frame is orthonormal to
        // round-off whatever the hint was.
        let width = (hint - length * hint.dot(&length)).normalize();
        let height = length.cross(&width);
        Ok(LocalBasis {
            length,
            width,
            height,
        })
    }
}

/// Index of a node within a [`Geometry`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub usize);

/// A segment as stored in a [`Geometry`]: its end points are references to
/// the geometry's nodes, which is what records that two segments meet.
///
/// [`Geometry::segment`] resolves it to a self-contained [`Segment`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SegmentDef {
    /// Node at the start of the centreline.
    pub a: NodeId,
    /// Node at the end of the centreline.
    pub b: NodeId,
    /// Cross-section extent along the width direction.
    pub width: f64,
    /// Cross-section extent along the height direction.
    pub height: f64,
    /// Electrical conductivity.
    pub sigma: f64,
    /// Optional explicit width direction; see [`Segment::width_dir`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_dir: Option<[f64; 3]>,
}

impl SegmentDef {
    /// Creates a segment definition with the default cross-section
    /// orientation.
    pub const fn new(a: NodeId, b: NodeId, width: f64, height: f64, sigma: f64) -> Self {
        Self {
            a,
            b,
            width,
            height,
            sigma,
            width_dir: None,
        }
    }

    /// Returns the definition with an explicit width direction.
    #[must_use]
    pub const fn with_width_dir(mut self, width_dir: [f64; 3]) -> Self {
        self.width_dir = Some(width_dir);
        self
    }
}

/// Why a [`Geometry`] was rejected.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum GeometryError {
    /// A node has a NaN or infinite coordinate.
    #[error("node {node} has a non-finite coordinate")]
    NonFiniteNode {
        /// Index of the offending node.
        node: usize,
    },
    /// A segment refers to a node the geometry does not contain.
    #[error(
        "segment {segment} refers to node {node}, but the geometry has only {node_count} nodes"
    )]
    DanglingNode {
        /// Index of the offending segment.
        segment: usize,
        /// The node index that does not exist.
        node: usize,
        /// Number of nodes in the geometry.
        node_count: usize,
    },
    /// A segment is not a valid conductor.
    #[error("segment {segment} is invalid: {reason}")]
    InvalidSegment {
        /// Index of the offending segment.
        segment: usize,
        /// What is wrong with it.
        reason: SegmentError,
    },
}

/// A validated set of nodes and the segments connecting them.
///
/// Every `Geometry` value is valid: [`add_node`](Self::add_node),
/// [`add_segment`](Self::add_segment), [`from_parts`](Self::from_parts) and
/// deserialization all reject non-finite nodes, dangling node references,
/// zero-length segments and non-positive dimensions, so the accessors are
/// infallible.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "GeometryParts")]
pub struct Geometry {
    nodes: Vec<Node>,
    segments: Vec<SegmentDef>,
}

/// Unvalidated wire form of a [`Geometry`], used to validate on deserialize.
#[derive(Deserialize)]
struct GeometryParts {
    #[serde(default)]
    nodes: Vec<Node>,
    #[serde(default)]
    segments: Vec<SegmentDef>,
}

impl TryFrom<GeometryParts> for Geometry {
    type Error = GeometryError;

    fn try_from(parts: GeometryParts) -> Result<Self, Self::Error> {
        Self::from_parts(parts.nodes, parts.segments)
    }
}

impl Geometry {
    /// Creates an empty geometry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a geometry from its nodes and segment definitions, validating
    /// all of them. Reports the first problem found.
    pub fn from_parts(nodes: Vec<Node>, segments: Vec<SegmentDef>) -> Result<Self, GeometryError> {
        let mut geometry = Self::new();
        for node in nodes {
            geometry.add_node(node)?;
        }
        for segment in segments {
            geometry.add_segment(segment)?;
        }
        Ok(geometry)
    }

    /// Adds a node and returns its id. Fails if a coordinate is not finite.
    pub fn add_node(&mut self, node: Node) -> Result<NodeId, GeometryError> {
        let index = self.nodes.len();
        if !node.is_finite() {
            return Err(GeometryError::NonFiniteNode { node: index });
        }
        self.nodes.push(node);
        Ok(NodeId(index))
    }

    /// Adds a segment between two existing nodes and returns its index.
    ///
    /// Fails — leaving the geometry unchanged — if either node id is
    /// dangling or the resolved segment does not pass
    /// [`Segment::validate`].
    pub fn add_segment(&mut self, def: SegmentDef) -> Result<usize, GeometryError> {
        let index = self.segments.len();
        let segment = self.resolve(index, &def)?;
        segment
            .validate()
            .map_err(|reason| GeometryError::InvalidSegment {
                segment: index,
                reason,
            })?;
        self.segments.push(def);
        Ok(index)
    }

    /// All nodes, indexable by [`NodeId`]`.0`.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The node with the given id, if it exists.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0)
    }

    /// All segment definitions, with their end points as node ids — the
    /// connectivity graph of the conductors.
    pub fn segment_defs(&self) -> &[SegmentDef] {
        &self.segments
    }

    /// Number of segments.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// The segment at `index`, resolved to node coordinates.
    pub fn segment(&self, index: usize) -> Option<Segment> {
        let def = self.segments.get(index)?;
        self.resolve(index, def).ok()
    }

    /// All segments in index order, resolved to node coordinates.
    pub fn segments(&self) -> impl Iterator<Item = Segment> + '_ {
        (0..self.segments.len()).filter_map(|index| self.segment(index))
    }

    fn resolve(&self, index: usize, def: &SegmentDef) -> Result<Segment, GeometryError> {
        let lookup = |id: NodeId| {
            self.node(id).copied().ok_or(GeometryError::DanglingNode {
                segment: index,
                node: id.0,
                node_count: self.nodes.len(),
            })
        };
        Ok(Segment {
            a: lookup(def.a)?,
            b: lookup(def.b)?,
            width: def.width,
            height: def.height,
            sigma: def.sigma,
            width_dir: def.width_dir,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COPPER: f64 = 5.8e7;

    fn assert_vec_close(actual: Vector3<f64>, expected: [f64; 3]) {
        let expected = Vector3::from(expected);
        assert!(
            (actual - expected).norm() < 1e-12,
            "expected {expected:?}, got {actual:?}"
        );
    }

    fn unit_x_segment() -> Segment {
        Segment::new(
            Node::new(0.0, 0.0, 0.0),
            Node::new(1.0, 0.0, 0.0),
            0.2,
            0.1,
            COPPER,
        )
    }

    #[test]
    fn segment_measures() {
        let s = Segment::new(
            Node::new(1.0, 2.0, 3.0),
            Node::new(4.0, 6.0, 3.0),
            0.2,
            0.1,
            COPPER,
        );
        assert!((s.length() - 5.0).abs() < 1e-15);
        assert!((s.area() - 0.02).abs() < 1e-15);
        assert_vec_close(s.center(), [2.5, 4.0, 3.0]);
    }

    #[test]
    fn default_basis_of_x_directed_segment_is_the_world_frame() {
        let basis = unit_x_segment().basis().unwrap();
        assert_vec_close(basis.length, [1.0, 0.0, 0.0]);
        assert_vec_close(basis.width, [0.0, 1.0, 0.0]);
        assert_vec_close(basis.height, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn default_basis_of_vertical_segment_takes_x_as_width() {
        let up = Segment::new(
            Node::new(0.0, 0.0, 0.0),
            Node::new(0.0, 0.0, 2.0),
            0.2,
            0.1,
            COPPER,
        );
        let basis = up.basis().unwrap();
        assert_vec_close(basis.length, [0.0, 0.0, 1.0]);
        assert_vec_close(basis.width, [1.0, 0.0, 0.0]);
        assert_vec_close(basis.height, [0.0, 1.0, 0.0]);

        let down = Segment {
            a: up.b,
            b: up.a,
            ..up
        };
        let basis = down.basis().unwrap();
        assert_vec_close(basis.length, [0.0, 0.0, -1.0]);
        assert_vec_close(basis.width, [1.0, 0.0, 0.0]);
        assert_vec_close(basis.height, [0.0, -1.0, 0.0]);
    }

    #[test]
    fn basis_is_right_handed_and_orthonormal_for_arbitrary_orientations() {
        let directions = [
            [1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 1.0, 0.0],
            [1.0, -2.0, 3.0],
            [-0.3, 0.4, -5.0],
            [1e-3, 0.0, 1.0],
            [1e-9, 0.0, 1.0],
        ];
        let hints = [None, Some([0.3, -0.2, 0.9]), Some([5.0, 1.0, 1.0])];
        for direction in directions {
            for hint in hints {
                let segment = Segment {
                    a: Node::new(0.5, -1.0, 2.0),
                    b: Node::from(Vector3::new(0.5, -1.0, 2.0) + Vector3::from(direction)),
                    width: 0.2,
                    height: 0.1,
                    sigma: COPPER,
                    width_dir: hint,
                };
                let LocalBasis {
                    length,
                    width,
                    height,
                } = segment.basis().unwrap();
                for unit in [length, width, height] {
                    assert!((unit.norm() - 1.0).abs() < 1e-12);
                }
                assert!(length.dot(&width).abs() < 1e-12);
                assert!(length.dot(&height).abs() < 1e-12);
                assert!(width.dot(&height).abs() < 1e-12);
                assert!((width.cross(&height) - length).norm() < 1e-12);
                assert!(
                    (length - Vector3::from(direction).normalize()).norm() < 1e-12,
                    "length direction must run from a to b"
                );
            }
        }
    }

    #[test]
    fn explicit_width_direction_is_projected_perpendicular_to_the_segment() {
        // The hint leans along the segment; only its perpendicular part counts.
        let segment = unit_x_segment().with_width_dir([7.0, 0.0, 2.0]);
        let basis = segment.basis().unwrap();
        assert_vec_close(basis.width, [0.0, 0.0, 1.0]);
        assert_vec_close(basis.height, [0.0, -1.0, 0.0]);
    }

    #[test]
    fn invalid_segments_are_rejected() {
        let good = unit_x_segment();
        assert_eq!(good.validate(), Ok(()));

        let zero_length = Segment { b: good.a, ..good };
        assert_eq!(zero_length.validate(), Err(SegmentError::ZeroLength));

        for width in [0.0, -0.2, f64::NAN, f64::INFINITY] {
            let err = Segment { width, ..good }.validate().unwrap_err();
            assert!(
                matches!(
                    err,
                    SegmentError::NonPositiveDimension {
                        dimension: "width",
                        ..
                    }
                ),
                "width {width}: {err:?}"
            );
        }
        for height in [0.0, -0.1, f64::NAN] {
            let err = Segment { height, ..good }.validate().unwrap_err();
            assert!(
                matches!(
                    err,
                    SegmentError::NonPositiveDimension {
                        dimension: "height",
                        ..
                    }
                ),
                "height {height}: {err:?}"
            );
        }
        for sigma in [0.0, -1.0, f64::NAN] {
            let err = Segment { sigma, ..good }.validate().unwrap_err();
            assert!(
                matches!(err, SegmentError::NonPositiveConductivity { .. }),
                "sigma {sigma}: {err:?}"
            );
        }

        let non_finite = Segment {
            b: Node::new(f64::NAN, 0.0, 0.0),
            ..good
        };
        assert_eq!(non_finite.validate(), Err(SegmentError::NonFiniteEndpoint));

        for dir in [[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [f64::NAN, 1.0, 0.0]] {
            assert_eq!(
                good.with_width_dir(dir).validate(),
                Err(SegmentError::InvalidWidthDirection),
                "width_dir {dir:?}"
            );
        }
    }

    fn two_segment_geometry() -> Geometry {
        let mut g = Geometry::new();
        let n0 = g.add_node(Node::new(0.0, 0.0, 0.0)).unwrap();
        let n1 = g.add_node(Node::new(1.0, 0.0, 0.0)).unwrap();
        let n2 = g.add_node(Node::new(1.0, 2.0, 0.0)).unwrap();
        assert_eq!(
            g.add_segment(SegmentDef::new(n0, n1, 0.2, 0.1, COPPER)),
            Ok(0)
        );
        assert_eq!(
            g.add_segment(
                SegmentDef::new(n1, n2, 0.3, 0.1, COPPER).with_width_dir([0.0, 0.0, 1.0])
            ),
            Ok(1)
        );
        g
    }

    #[test]
    fn geometry_resolves_segments_to_node_coordinates() {
        let g = two_segment_geometry();
        assert_eq!(g.nodes().len(), 3);
        assert_eq!(g.segment_count(), 2);
        assert_eq!(g.segment_defs()[1].a, NodeId(1));
        assert_eq!(g.node(NodeId(2)), Some(&Node::new(1.0, 2.0, 0.0)));
        assert_eq!(g.node(NodeId(3)), None);

        let segments: Vec<Segment> = g.segments().collect();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0], unit_x_segment());
        assert_eq!(
            segments[1],
            Segment::new(
                Node::new(1.0, 0.0, 0.0),
                Node::new(1.0, 2.0, 0.0),
                0.3,
                0.1,
                COPPER
            )
            .with_width_dir([0.0, 0.0, 1.0])
        );
        assert_eq!(g.segment(1), Some(segments[1]));
        assert_eq!(g.segment(2), None);
    }

    #[test]
    fn geometry_rejects_dangling_node_references() {
        let mut g = two_segment_geometry();
        let before = g.clone();
        let err = g
            .add_segment(SegmentDef::new(NodeId(0), NodeId(7), 0.2, 0.1, COPPER))
            .unwrap_err();
        assert_eq!(
            err,
            GeometryError::DanglingNode {
                segment: 2,
                node: 7,
                node_count: 3
            }
        );
        assert_eq!(g, before, "a rejected segment must not be stored");
    }

    #[test]
    fn geometry_rejects_zero_length_and_non_positive_dimensions() {
        let mut g = two_segment_geometry();
        // Two distinct nodes at the same place still make a zero-length segment.
        let twin = g.add_node(Node::new(0.0, 0.0, 0.0)).unwrap();

        assert_eq!(
            g.add_segment(SegmentDef::new(NodeId(0), NodeId(0), 0.2, 0.1, COPPER)),
            Err(GeometryError::InvalidSegment {
                segment: 2,
                reason: SegmentError::ZeroLength
            })
        );
        assert_eq!(
            g.add_segment(SegmentDef::new(NodeId(0), twin, 0.2, 0.1, COPPER)),
            Err(GeometryError::InvalidSegment {
                segment: 2,
                reason: SegmentError::ZeroLength
            })
        );
        assert_eq!(
            g.add_segment(SegmentDef::new(NodeId(0), NodeId(1), 0.2, -0.1, COPPER)),
            Err(GeometryError::InvalidSegment {
                segment: 2,
                reason: SegmentError::NonPositiveDimension {
                    dimension: "height",
                    value: -0.1
                }
            })
        );
        assert_eq!(g.segment_count(), 2);
    }

    #[test]
    fn geometry_rejects_non_finite_nodes() {
        let mut g = Geometry::new();
        assert_eq!(
            g.add_node(Node::new(0.0, f64::INFINITY, 0.0)),
            Err(GeometryError::NonFiniteNode { node: 0 })
        );
        assert!(g.nodes().is_empty());
    }

    #[test]
    fn geometry_error_messages_name_the_offender() {
        let err = GeometryError::InvalidSegment {
            segment: 4,
            reason: SegmentError::ZeroLength,
        };
        assert_eq!(
            err.to_string(),
            "segment 4 is invalid: segment has zero length (its end points coincide)"
        );
        let err = GeometryError::DanglingNode {
            segment: 1,
            node: 9,
            node_count: 2,
        };
        assert_eq!(
            err.to_string(),
            "segment 1 refers to node 9, but the geometry has only 2 nodes"
        );
    }

    #[test]
    fn geometry_round_trips_through_json() {
        let g = two_segment_geometry();
        let json = serde_json::to_string(&g).unwrap();
        let back: Geometry = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);
    }

    #[test]
    fn geometry_json_shape_is_stable() {
        let json = r#"{
            "nodes": [{"x": 0, "y": 0, "z": 0}, {"x": 1, "y": 0, "z": 0}],
            "segments": [{"a": 0, "b": 1, "width": 0.2, "height": 0.1, "sigma": 5.8e7}]
        }"#;
        let g: Geometry = serde_json::from_str(json).unwrap();
        assert_eq!(g.segment(0), Some(unit_x_segment()));
    }

    #[test]
    fn deserializing_an_invalid_geometry_fails() {
        let dangling = r#"{
            "nodes": [{"x": 0, "y": 0, "z": 0}],
            "segments": [{"a": 0, "b": 1, "width": 0.2, "height": 0.1, "sigma": 5.8e7}]
        }"#;
        let err = serde_json::from_str::<Geometry>(dangling).unwrap_err();
        assert!(err.to_string().contains("refers to node 1"), "{err}");

        let flat = r#"{
            "nodes": [{"x": 0, "y": 0, "z": 0}, {"x": 1, "y": 0, "z": 0}],
            "segments": [{"a": 0, "b": 1, "width": 0.0, "height": 0.1, "sigma": 5.8e7}]
        }"#;
        let err = serde_json::from_str::<Geometry>(flat).unwrap_err();
        assert!(err.to_string().contains("width must be positive"), "{err}");
    }

    #[test]
    fn segment_round_trips_through_json() {
        let s = unit_x_segment().with_width_dir([0.0, 1.0, 1.0]);
        let back: Segment = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}
