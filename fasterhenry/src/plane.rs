//! Ground planes: thick rectangular sheets discretized into a mesh of
//! bars, with rectangular holes — the FastHenry `G` feature, on the same
//! segment/kernel/mesh machinery everything else uses.
//!
//! # Discretization
//!
//! A plane occupies `[x_lo, x_hi] × [y_lo, y_hi]` at surface `z_top`,
//! extending downward `thickness`; it is cut into `nx × ny` cells. The
//! mesh places one node at each **cell centre** (at the mid-thickness
//! depth) and one bar between every pair of horizontally- or
//! vertically-adjacent centres:
//!
//! * an x-directed bar spans one cell width `dx`, with cross-section
//!   `dy × thickness`;
//! * a y-directed bar spans `dy`, with cross-section `dx × thickness`.
//!
//! Cells whose centre lies inside a hole are removed together with their
//! incident bars. This is a PEEC discretization in its own right (not a
//! transcription of FastHenry's particular panel mesh — see the
//! clean-room note in the repository README): convergence to the
//! continuum plane goes as the grid is refined.
//!
//! # Connection
//!
//! A segment endpoint that lands within a plane's footprint and depth is
//! **snapped** to the nearest live cell-centre node ([`attach`]): the
//! segment then shares that node with the plane mesh, closing the current
//! path. Snapping is explicit in the API and in the deck reader (`G`'
//! footprint), never silent elsewhere.
//!
//! # Cost
//!
//! Bars grow as `2·nx·ny`; each becomes at least one filament, so the
//! dense solve cost is cubic in the mesh. A 30 × 30 plane is ~1 740 bars
//! — comfortably in the dense regime; larger planes are the FFT issue's
//! motivation.

use thiserror::Error;

use crate::geometry::{Geometry, GeometryError, Node, NodeId, SegmentDef};

/// A rectangular hole in a plane's surface coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hole {
    /// Inclusive lower corner `(x, y)`.
    pub lo: [f64; 2],
    /// Exclusive... see [`contains`]: a cell centre strictly inside.
    pub hi: [f64; 2],
}

/// A ground plane specification: extent, discretization, holes.
#[derive(Clone, Debug, Default)]
pub struct GroundPlane {
    /// Lower corner of the footprint, metres.
    pub lo: [f64; 2],
    /// Upper corner of the footprint, metres.
    pub hi: [f64; 2],
    /// Top surface z, metres; the plane extends downward by `thickness`.
    pub z_top: f64,
    /// Thickness, metres.
    pub thickness: f64,
    /// Cells across x.
    pub nx: usize,
    /// Cells across y.
    pub ny: usize,
    /// Conductivity, S/m.
    pub sigma: f64,
    /// Rectangular holes (x/y in plane coordinates).
    pub holes: Vec<Hole>,
}

/// Why a ground plane could not be built.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum PlaneError {
    /// The discretization is degenerate.
    #[error("ground plane needs nx, ny >= 1 (got {nx}, {ny})")]
    ZeroSubdivision {
        /// The requested x cell count.
        nx: usize,
        /// The requested y cell count.
        ny: usize,
    },
    /// The extent is not a proper rectangle.
    #[error("ground plane extent is degenerate (lo {lo:?}, hi {hi:?})")]
    DegenerateExtent {
        /// Lower corner.
        lo: [f64; 2],
        /// Upper corner.
        hi: [f64; 2],
    },
    /// Inherited from the geometry builder.
    #[error("{source}")]
    Geometry {
        /// The underlying error.
        #[from]
        source: GeometryError,
    },
}

impl GroundPlane {
    /// Cell size `(dx, dy)` in metres.
    fn cell(&self) -> (f64, f64) {
        (
            (self.hi[0] - self.lo[0]) / self.nx as f64,
            (self.hi[1] - self.lo[1]) / self.ny as f64,
        )
    }

    /// The centre of cell `(i, j)`, at the mid-thickness depth.
    fn centre(&self, i: usize, j: usize) -> [f64; 3] {
        let (dx, dy) = self.cell();
        [
            self.lo[0] + (i as f64 + 0.5) * dx,
            self.lo[1] + (j as f64 + 0.5) * dy,
            self.z_top - self.thickness / 2.0,
        ]
    }

    /// Whether cell `(i, j)`'s centre falls inside a hole.
    fn holed(&self, i: usize, j: usize) -> bool {
        let centre = self.centre(i, j);
        self.holes.iter().any(|hole| {
            hole.lo[0] < centre[0]
                && centre[0] < hole.hi[0]
                && hole.lo[1] < centre[1]
                && centre[1] < hole.hi[1]
        })
    }

    /// Builds the plane's mesh into `geometry`, returning the node id of
    /// each live cell centre, indexed `[i][j]` (`None` where a hole
    /// removed the cell).
    ///
    /// # Errors
    ///
    /// See [`PlaneError`].
    pub fn build_into(&self, geometry: &mut Geometry) -> Result<Vec<Vec<Option<NodeId>>>, PlaneError> {
        if self.nx < 1 || self.ny < 1 {
            return Err(PlaneError::ZeroSubdivision { nx: self.nx, ny: self.ny });
        }
        if !(self.hi[0] > self.lo[0] && self.hi[1] > self.lo[1]) {
            return Err(PlaneError::DegenerateExtent { lo: self.lo, hi: self.hi });
        }
        let (dx, dy) = self.cell();
        let cross_x = dy * self.thickness; // x-bar cross-section
        let cross_y = dx * self.thickness; // y-bar cross-section

        let mut centres = vec![vec![None; self.ny]; self.nx];
        for i in 0..self.nx {
            for j in 0..self.ny {
                if self.holed(i, j) {
                    continue;
                }
                let position = self.centre(i, j);
                let node = geometry.add_node(Node::new(position[0], position[1], position[2]))?;
                centres[i][j] = Some(node);
            }
        }
        let mut bars = 0;
        for i in 0..self.nx {
            for j in 0..self.ny {
                let here = match centres[i][j] {
                    Some(node) => node,
                    None => continue,
                };
                // x-direction bar to the right neighbour.
                if i + 1 < self.nx {
                    if let Some(right) = centres[i + 1][j] {
                        geometry.add_segment(SegmentDef::new(here, right, cross_x, self.thickness, self.sigma))?;
                        bars += 1;
                    }
                }
                // y-direction bar upward.
                if j + 1 < self.ny {
                    if let Some(up) = centres[i][j + 1] {
                        geometry.add_segment(SegmentDef::new(here, up, cross_y, self.thickness, self.sigma))?;
                        bars += 1;
                    }
                }
            }
        }
        debug_assert_eq!(
            bars,
            self.live_bars(),
            "bar count matches the independent count"
        );
        Ok(centres)
    }

    /// Independent count of live bars (for the debug assertion above and
    /// for tests).
    fn live_bars(&self) -> usize {
        let live = |i: usize, j: usize| !self.holed(i, j);
        let mut bars = 0;
        for i in 0..self.nx {
            for j in 0..self.ny {
                if live(i, j) && i + 1 < self.nx && live(i + 1, j) {
                    bars += 1;
                }
                if live(i, j) && j + 1 < self.ny && live(i, j + 1) {
                    bars += 1;
                }
            }
        }
        bars
    }

    /// Whether a point is inside the plane's metal footprint and depth
    /// (with `tolerance` metres of slack on every side, for endpoints
    /// exactly on the surface).
    pub fn contains(&self, point: [f64; 3], tolerance: f64) -> bool {
        let z_bottom = self.z_top - self.thickness;
        point[0] >= self.lo[0] - tolerance
            && point[0] <= self.hi[0] + tolerance
            && point[1] >= self.lo[1] - tolerance
            && point[1] <= self.hi[1] + tolerance
            && point[2] <= self.z_top + tolerance
            && point[2] >= z_bottom - tolerance
    }

    /// The live cell-centre node nearest to `point`, for connecting a
    /// segment: the plane's mesh into which the snapped endpoint's segment
    /// should merge. `Err` carries whether the point was outside the plane
    /// at all (when it was, the caller should not snap).
    ///
    /// # Errors
    ///
    /// [`PlaneError::DegenerateExtent`] if the plane has no live cells.
    pub fn attach(
        &self,
        centres: &[Vec<Option<NodeId>>],
        point: [f64; 3],
    ) -> Result<NodeId, PlaneError> {
        let mut best: Option<(f64, NodeId)> = None;
        for (i, column) in centres.iter().enumerate() {
            for (j, node) in column.iter().enumerate() {
                let Some(node) = *node else { continue };
                let centre = self.centre(i, j);
                let distance = (centre[0] - point[0]).hypot(centre[1] - point[1]);
                if best.is_none_or(|(current, _)| distance < current) {
                    best = Some((distance, node));
                }
            }
        }
        best.map(|(_, node)| node)
            .ok_or(PlaneError::DegenerateExtent { lo: self.lo, hi: self.hi })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_plane() -> GroundPlane {
        GroundPlane {
            lo: [0.0, 0.0],
            hi: [10e-3, 6e-3],
            z_top: 0.0,
            thickness: 35e-6,
            nx: 5,
            ny: 3,
            sigma: 5.8e7,
            holes: Vec::new(),
        }
    }

    #[test]
    fn mesh_counts_and_positions() {
        let mut geometry = Geometry::new();
        let plane = test_plane();
        let centres = plane.build_into(&mut geometry).unwrap();
        // 5x3 cells, no holes: 15 nodes, 4*3 + 5*2 = 22 bars.
        assert_eq!(geometry.nodes().len(), 15);
        assert_eq!(geometry.segment_count(), 22);
        // First centre is half a cell in from the corner, mid-thickness.
        let first = centres[0][0].unwrap();
        let node = geometry.nodes()[first.0];
        assert!((node.x - 1e-3).abs() < 1e-12);
        assert!((node.y - 1e-3).abs() < 1e-12);
        assert!((node.z + 17.5e-6).abs() < 1e-12);
        // Every cell is live.
        assert!(centres.iter().flatten().all(Option::is_some));
    }

    #[test]
    fn holes_remove_cells_and_bars() {
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        // A hole covering the middle cell (2,1): its centre (5mm, 3mm).
        plane.holes.push(Hole { lo: [4.9e-3, 2.9e-3], hi: [5.1e-3, 3.1e-3] });
        let centres = plane.build_into(&mut geometry).unwrap();
        assert!(centres[2][1].is_none());
        assert_eq!(geometry.nodes().len(), 14);
        // 22 bars minus the 4 incident to the removed cell.
        assert_eq!(geometry.segment_count(), 18);
        // A hole exactly on a centre boundary does not remove the cell.
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        plane.holes.push(Hole { lo: [5e-3, 0.0], hi: [6e-3, 6e-3] });
        plane.build_into(&mut geometry).unwrap();
        assert_eq!(geometry.nodes().len(), 15);
    }

    #[test]
    fn attach_snaps_to_the_nearest_live_centre() {
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        // A hole over the (0, 0) cell's centre (1 mm, 1 mm): that cell
        // dies; a point over the holed corner then snaps to the nearest
        // live centre, cell (0, 1) at (1 mm, 3 mm) — ties resolved by
        // grid order.
        plane.holes.push(Hole { lo: [0.0, 0.0], hi: [2.0e-3, 2.0e-3] });
        let centres = plane.build_into(&mut geometry).unwrap();
        assert!(centres[0][0].is_none());
        let snapped = plane.attach(&centres, [0.5e-3, 0.5e-3, 0.0]).unwrap();
        assert_eq!(snapped, centres[0][1].unwrap());
        let node = geometry.nodes()[snapped.0];
        assert!((node.x - 1e-3).abs() < 1e-12);
        assert!((node.y - 3e-3).abs() < 1e-12);
    }

    #[test]
    fn contains_respects_footprint_depth_and_tolerance() {
        let plane = test_plane();
        assert!(plane.contains([1e-3, 1e-3, 0.0], 0.0));
        assert!(plane.contains([1e-3, 1e-3, -35e-6], 0.0));
        assert!(plane.contains([10e-3, 1e-3, 0.0], 1e-15));
        assert!(!plane.contains([10e-3 + 1e-9, 1e-3, 0.0], 1e-15));
        assert!(!plane.contains([1e-3, 1e-3, 1e-9], 0.0));
        assert!(!plane.contains([1e-3, 1e-3, -35e-6 - 1e-9], 0.0));
    }

    #[test]
    fn errors_for_degenerate_specifications() {
        let mut geometry = Geometry::new();
        let error = GroundPlane { nx: 0, ny: 3, ..test_plane() }.build_into(&mut geometry).unwrap_err();
        assert_eq!(error, PlaneError::ZeroSubdivision { nx: 0, ny: 3 });
        let error = GroundPlane { hi: [0.0, 6e-3], ..test_plane() }.build_into(&mut geometry).unwrap_err();
        assert!(matches!(error, PlaneError::DegenerateExtent { .. }));
    }
}
