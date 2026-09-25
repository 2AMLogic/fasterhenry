//! Ground planes: thick rectangular sheets discretized into a mesh of
//! bars, with rectangular holes and locally refined contact regions — the
//! FastHenry `G` feature, on the same segment/kernel/mesh machinery
//! everything else uses.
//!
//! # Discretization
//!
//! A plane occupies `[x_lo, x_hi] × [y_lo, y_hi]` at surface `z_top`,
//! extending downward `thickness`. Its **background** resolution is
//! `nx × ny` cells; with no [`ContactRegion`] those cells are uniform.
//! The mesh places one node at each **cell centre** (at the mid-thickness
//! depth) and one bar between every pair of horizontally- or
//! vertically-adjacent centres:
//!
//! * an x-directed bar spans centre to centre — `(dxᵢ + dxᵢ₊₁)/2` — with
//!   cross-section `dyⱼ × thickness`;
//! * a y-directed bar spans `(dyⱼ + dyⱼ₊₁)/2`, with cross-section
//!   `dxᵢ × thickness`.
//!
//! Cells whose centre lies inside a hole are removed together with their
//! incident bars. This is a PEEC discretization in its own right (not a
//! transcription of FastHenry's particular panel mesh — see the
//! clean-room note in the repository README): convergence to the
//! continuum plane goes as the grid is refined.
//!
//! # Graded contact regions
//!
//! A via landing pulls the return current into a small patch of the
//! plane, and that patch is where the mesh has to be fine: a uniform
//! cell-centre grid converges only like `1/n`, so resolving a landing by
//! refining everywhere is ruinous. A [`ContactRegion`] refines locally
//! instead — inside the region the mesh is uniform at the fine cell
//! `h = w / cells`, and outside it each cell is `ratio` times its
//! neighbour until it reaches the background cell `c`.
//!
//! Grading is applied **per axis**, so the mesh stays a structured
//! (tensor-product) grid: a region's x-refinement extends as a band
//! across the whole plane in y, and its y-refinement as a band across x.
//! That is the price of a rectangular node array with conforming bars and
//! no hanging nodes; it still buys most of the saving, because the bands
//! are narrow. Regions compose with holes (a hole inside a refined region
//! removes its cells exactly as anywhere else) and with each other
//! (regions overlapping — or closer than one fine cell — on an axis merge
//! into one band, keeping the finest cell and the gentlest ratio).
//!
//! ## Cell count
//!
//! Per axis, for a span `W` with background count `n` (coarse cell
//! `c = W/n`) and one region of width `w` cut into `k` cells
//! (`h = w/k`) at ratio `r`: the region contributes `k` cells, and each
//! gap of length `g` beside it takes the smallest `m` cells with
//!
//! ```text
//! Σ_{i<m} min(h·r^(i+1), c)  ≥  g
//! ```
//!
//! — those `m` target extents, scaled to fill `g` exactly. A gap with a
//! region on *both* sides takes the smaller of the two sides' targets per
//! cell, so it is thin at both ends and coarse in the middle. The axis
//! total is `k + Σ m` cells.
//!
//! # Cost
//!
//! Bars grow as `2·nx·ny − nx − ny` for an `nx × ny` mesh; each becomes
//! at least one filament, so the dense solve cost is cubic in the mesh. A
//! 30 × 30 plane is ~1 740 bars — comfortably in the dense regime; larger
//! planes are the FFT issue's motivation. Grading is what keeps a
//! contact-resolving plane in that regime: on a 1.2 × 0.8 mm plane,
//! resolving two 0.125 mm landings at 25 µm costs **580 bars** graded at
//! `r = 2` from a 100 µm background, against **2 992** for the uniform
//! 25 µm plane — and the two agree to 0.10 % on inductance and 0.80 % on
//! resistance (measured, `fasterhenry/tests/plane_validation.rs`;
//! `docs/validation.md` § *Graded contact regions*). The unrefined 100 µm
//! background is 12.5 % off on the same fixture.
//!
//! # Connection
//!
//! A segment endpoint that lands within a plane's footprint and depth is
//! **snapped** to the nearest live cell-centre node ([`GroundPlane::attach`]): the
//! segment then shares that node with the plane mesh, closing the current
//! path. Snapping is explicit in the API and in the deck reader (`G`'
//! footprint), never silent elsewhere.

use thiserror::Error;

use crate::geometry::{Geometry, GeometryError, Node, NodeId, SegmentDef};

/// A rectangular hole in a plane's surface coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hole {
    /// Inclusive lower corner `(x, y)`.
    pub lo: [f64; 2],
    /// Exclusive... see [`GroundPlane::contains`]: a cell centre strictly inside.
    pub hi: [f64; 2],
}

/// A rectangular contact region: the patch of plane under a via landing,
/// meshed finely and decaying back to the background cell outside.
///
/// Inside `[lo, hi]` the mesh is uniform with `cells` cells per axis (fine
/// cell `h = (hi − lo) / cells`); outside, cells grow by `ratio` each until
/// they reach the background cell. See the [module
/// documentation](self#graded-contact-regions) for the tensor-product
/// consequence and the cell-count formula.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactRegion {
    /// Lower corner `(x, y)` in plane coordinates, metres.
    pub lo: [f64; 2],
    /// Upper corner `(x, y)`, metres.
    pub hi: [f64; 2],
    /// Fine cells across the region, `[x, y]`.
    pub cells: [usize; 2],
    /// Geometric growth ratio per cell outside the region (`>= 1`; `1` is
    /// no decay at all, i.e. a uniformly fine axis).
    pub ratio: f64,
}

impl ContactRegion {
    /// A region spanning `lo … hi`, `cells` fine cells per axis, decaying
    /// outward by `ratio`.
    #[must_use]
    pub fn new(lo: [f64; 2], hi: [f64; 2], cells: [usize; 2], ratio: f64) -> Self {
        Self {
            lo,
            hi,
            cells,
            ratio,
        }
    }

    /// A square region of side `side` centred on `(x, y)`, `cells` fine
    /// cells per axis, decaying outward by `ratio` — the via-landing case.
    #[must_use]
    pub fn centred(centre: [f64; 2], side: f64, cells: usize, ratio: f64) -> Self {
        let half = side / 2.0;
        Self::new(
            [centre[0] - half, centre[1] - half],
            [centre[0] + half, centre[1] + half],
            [cells, cells],
            ratio,
        )
    }
}

/// A ground plane specification: extent, discretization, holes, contacts.
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
    /// Background cells across x (the resolution away from every contact
    /// region).
    pub nx: usize,
    /// Background cells across y.
    pub ny: usize,
    /// Conductivity, S/m.
    pub sigma: f64,
    /// Rectangular holes (x/y in plane coordinates).
    pub holes: Vec<Hole>,
    /// Locally refined contact regions (x/y in plane coordinates).
    pub contacts: Vec<ContactRegion>,
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
    /// A contact region is not a proper rectangle.
    #[error("contact region extent is degenerate (lo {lo:?}, hi {hi:?})")]
    ContactExtent {
        /// Lower corner.
        lo: [f64; 2],
        /// Upper corner.
        hi: [f64; 2],
    },
    /// A contact region does not overlap the plane at all.
    #[error("contact region (lo {lo:?}, hi {hi:?}) lies outside the plane footprint")]
    ContactOutsideFootprint {
        /// Lower corner.
        lo: [f64; 2],
        /// Upper corner.
        hi: [f64; 2],
    },
    /// A contact region asks for no cells on some axis.
    #[error("contact region needs cells >= 1 on both axes (got {cells:?})")]
    ContactZeroCells {
        /// The requested fine cell counts.
        cells: [usize; 2],
    },
    /// A contact region's decay ratio is unusable.
    #[error("contact decay ratio must be finite and >= 1 (got {ratio})")]
    ContactRatio {
        /// The requested ratio.
        ratio: f64,
    },
    /// The graded layout would need an absurd number of cells.
    #[error("graded plane mesh needs more than {limit} cells on one axis: coarsen the contacts")]
    MeshTooLarge {
        /// The per-axis cell limit.
        limit: usize,
    },
    /// Inherited from the geometry builder.
    #[error("{source}")]
    Geometry {
        /// The underlying error.
        #[from]
        source: GeometryError,
    },
}

/// Upper bound on the cells one axis may hold, so a pathologically fine
/// contact region reports an error instead of exhausting memory.
const MAX_AXIS_CELLS: usize = 100_000;

/// A refined band on one axis: `start … end` cut into `cells` uniform
/// cells, with the cells outside growing by `ratio` each.
#[derive(Clone, Copy, Debug)]
struct Band {
    start: f64,
    end: f64,
    cells: usize,
    ratio: f64,
}

impl Band {
    /// The band's uniform cell extent.
    fn fine(&self) -> f64 {
        (self.end - self.start) / self.cells as f64
    }
}

/// A plane's cell layout: the edges its cell-centre mesh sits on.
///
/// Uniform for a plane without [`ContactRegion`]s; graded around each
/// region otherwise. Obtained from [`GroundPlane::mesh`].
#[derive(Clone, Debug, PartialEq)]
pub struct PlaneMesh {
    /// Cell edges across x, ascending, `nx + 1` of them.
    x: Vec<f64>,
    /// Cell edges across y, ascending, `ny + 1` of them.
    y: Vec<f64>,
    /// The mid-thickness depth every node sits at.
    z: f64,
}

impl PlaneMesh {
    /// Cells across x.
    #[must_use]
    pub fn nx(&self) -> usize {
        self.x.len() - 1
    }

    /// Cells across y.
    #[must_use]
    pub fn ny(&self) -> usize {
        self.y.len() - 1
    }

    /// Cell edges across x, ascending (`nx() + 1` coordinates).
    #[must_use]
    pub fn x_edges(&self) -> &[f64] {
        &self.x
    }

    /// Cell edges across y, ascending (`ny() + 1` coordinates).
    #[must_use]
    pub fn y_edges(&self) -> &[f64] {
        &self.y
    }

    /// Extent of cell column `i` across x.
    ///
    /// # Panics
    ///
    /// If `i >= self.nx()`.
    #[must_use]
    pub fn dx(&self, i: usize) -> f64 {
        self.x[i + 1] - self.x[i]
    }

    /// Extent of cell row `j` across y.
    ///
    /// # Panics
    ///
    /// If `j >= self.ny()`.
    #[must_use]
    pub fn dy(&self, j: usize) -> f64 {
        self.y[j + 1] - self.y[j]
    }

    /// The centre of cell `(i, j)`, at the mid-thickness depth.
    ///
    /// # Panics
    ///
    /// If `i >= self.nx()` or `j >= self.ny()`.
    #[must_use]
    pub fn centre(&self, i: usize, j: usize) -> [f64; 3] {
        [
            (self.x[i] + self.x[i + 1]) / 2.0,
            (self.y[j] + self.y[j + 1]) / 2.0,
            self.z,
        ]
    }

    /// Bars the mesh would carry with every cell live:
    /// `2·nx·ny − nx − ny`.
    #[must_use]
    pub fn bars(&self) -> usize {
        2 * self.nx() * self.ny() - self.nx() - self.ny()
    }
}

/// Appends the cells filling the gap `from … to` to `edges`.
///
/// `left` and `right` carry the `(fine, ratio)` of the refined band
/// bordering the gap on that side, `None` at a plane edge. Cell extents
/// grow by `ratio` per cell away from each refined side and level off at
/// `coarse`; the count is the smallest that covers the gap, and the
/// extents are then scaled to fill it exactly.
fn push_gap(
    edges: &mut Vec<f64>,
    from: f64,
    to: f64,
    left: Option<(f64, f64)>,
    right: Option<(f64, f64)>,
    coarse: f64,
) -> Result<(), PlaneError> {
    let gap = to - from;
    if gap <= 0.0 {
        return Ok(());
    }
    // The k-th target extent away from a refined side, capped at `coarse`;
    // `None` (a plane edge) constrains nothing.
    let target = |side: Option<(f64, f64)>, k: usize| -> f64 {
        match side {
            Some((fine, ratio)) => (fine * ratio.powi(k as i32 + 1)).min(coarse),
            None => f64::INFINITY,
        }
    };
    let weight = |i: usize, count: usize| -> f64 {
        let extent = target(left, i).min(target(right, count - 1 - i));
        // Bordered by no refined band at all: plain background cells.
        if extent.is_finite() {
            extent
        } else {
            coarse
        }
    };
    let covered = |count: usize| -> f64 { (0..count).map(|i| weight(i, count)).sum() };
    // `covered` is nondecreasing in `count` (the target sequence is
    // nondecreasing), so double until the gap is covered and bisect.
    let mut upper = 1usize;
    while covered(upper) < gap {
        if upper > MAX_AXIS_CELLS {
            return Err(PlaneError::MeshTooLarge {
                limit: MAX_AXIS_CELLS,
            });
        }
        upper *= 2;
    }
    let mut lower = 1usize;
    while lower < upper {
        let mid = lower + (upper - lower) / 2;
        if covered(mid) >= gap {
            upper = mid;
        } else {
            lower = mid + 1;
        }
    }
    let count = upper;
    let weights: Vec<f64> = (0..count).map(|i| weight(i, count)).collect();
    let total: f64 = weights.iter().sum();
    let mut cursor = from;
    for weight in weights {
        cursor += gap * weight / total;
        edges.push(cursor);
    }
    // The accumulation is inexact; the gap's far edge is not.
    *edges.last_mut().expect("the gap has at least one cell") = to;
    Ok(())
}

/// Cell edges along one axis: `lo … hi` at `background` uniform cells,
/// refined inside every band and geometrically graded outside them.
fn axis_edges(
    lo: f64,
    hi: f64,
    background: usize,
    mut bands: Vec<Band>,
) -> Result<Vec<f64>, PlaneError> {
    let coarse = (hi - lo) / background as f64;
    if bands.is_empty() {
        let mut edges: Vec<f64> = (0..background).map(|i| lo + i as f64 * coarse).collect();
        edges.push(hi);
        return Ok(edges);
    }
    bands.sort_by(|a, b| a.start.total_cmp(&b.start));
    // Bands that overlap — or sit closer than one fine cell — become one
    // band, keeping the finest cell and the gentlest ratio.
    let mut merged: Vec<Band> = Vec::with_capacity(bands.len());
    for band in bands {
        match merged.last_mut() {
            Some(last) if band.start <= last.end + last.fine().min(band.fine()) => {
                let fine = last.fine().min(band.fine());
                last.end = last.end.max(band.end);
                last.ratio = last.ratio.min(band.ratio);
                last.cells = (((last.end - last.start) / fine).round() as usize).max(1);
            }
            _ => merged.push(band),
        }
    }
    // The bands' own cells are counted *before* anything is laid out: a
    // region asking for more of them than the axis may hold has to be an
    // error, not an allocation (`Band::cells` is derived from a
    // caller-supplied count, so it can be arbitrarily large).
    let band_cells = merged
        .iter()
        .try_fold(0usize, |total, band| total.checked_add(band.cells));
    if band_cells.is_none_or(|total| total > MAX_AXIS_CELLS) {
        return Err(PlaneError::MeshTooLarge {
            limit: MAX_AXIS_CELLS,
        });
    }
    let mut edges = vec![lo];
    let mut cursor = lo;
    for (index, band) in merged.iter().enumerate() {
        let previous = index.checked_sub(1).map(|p| &merged[p]);
        push_gap(
            &mut edges,
            cursor,
            band.start,
            previous.map(|b| (b.fine(), b.ratio)),
            Some((band.fine(), band.ratio)),
            coarse,
        )?;
        let fine = band.fine();
        for k in 1..=band.cells {
            edges.push(band.start + k as f64 * fine);
        }
        *edges.last_mut().expect("the band has at least one cell") = band.end;
        cursor = band.end;
        // Several bands, each individually affordable, must not add up to
        // an unaffordable axis either.
        if edges.len() > MAX_AXIS_CELLS {
            return Err(PlaneError::MeshTooLarge {
                limit: MAX_AXIS_CELLS,
            });
        }
    }
    let last = merged.last().expect("at least one band");
    push_gap(
        &mut edges,
        cursor,
        hi,
        Some((last.fine(), last.ratio)),
        None,
        coarse,
    )?;
    if edges.len() - 1 > MAX_AXIS_CELLS {
        return Err(PlaneError::MeshTooLarge {
            limit: MAX_AXIS_CELLS,
        });
    }
    Ok(edges)
}

impl GroundPlane {
    /// The plane's cell layout — uniform `nx × ny`, refined around every
    /// [`ContactRegion`].
    ///
    /// # Errors
    ///
    /// See [`PlaneError`]: a degenerate extent or subdivision, an unusable
    /// or non-overlapping contact region, or a layout beyond the per-axis
    /// cell limit.
    pub fn mesh(&self) -> Result<PlaneMesh, PlaneError> {
        if self.nx < 1 || self.ny < 1 {
            return Err(PlaneError::ZeroSubdivision {
                nx: self.nx,
                ny: self.ny,
            });
        }
        if !(self.hi[0] > self.lo[0] && self.hi[1] > self.lo[1]) {
            return Err(PlaneError::DegenerateExtent {
                lo: self.lo,
                hi: self.hi,
            });
        }
        let mut bands: [Vec<Band>; 2] = [Vec::new(), Vec::new()];
        for contact in &self.contacts {
            if !(contact.ratio.is_finite() && contact.ratio >= 1.0) {
                return Err(PlaneError::ContactRatio {
                    ratio: contact.ratio,
                });
            }
            if contact.cells[0] < 1 || contact.cells[1] < 1 {
                return Err(PlaneError::ContactZeroCells {
                    cells: contact.cells,
                });
            }
            if !(contact.hi[0] > contact.lo[0] && contact.hi[1] > contact.lo[1]) {
                return Err(PlaneError::ContactExtent {
                    lo: contact.lo,
                    hi: contact.hi,
                });
            }
            for (axis, bands) in bands.iter_mut().enumerate() {
                // The fine cell follows the region as declared, so a
                // region straddling the footprint edge keeps its
                // resolution; the band itself is clipped to the plane.
                let fine = (contact.hi[axis] - contact.lo[axis]) / contact.cells[axis] as f64;
                let start = contact.lo[axis].max(self.lo[axis]);
                let end = contact.hi[axis].min(self.hi[axis]);
                if end <= start {
                    return Err(PlaneError::ContactOutsideFootprint {
                        lo: contact.lo,
                        hi: contact.hi,
                    });
                }
                // A leftover strip thinner than one fine cell is absorbed
                // into the band rather than left as a sliver cell.
                let start = if start - self.lo[axis] < fine {
                    self.lo[axis]
                } else {
                    start
                };
                let end = if self.hi[axis] - end < fine {
                    self.hi[axis]
                } else {
                    end
                };
                bands.push(Band {
                    start,
                    end,
                    cells: (((end - start) / fine).round() as usize).max(1),
                    ratio: contact.ratio,
                });
            }
        }
        let [x_bands, y_bands] = bands;
        Ok(PlaneMesh {
            x: axis_edges(self.lo[0], self.hi[0], self.nx, x_bands)?,
            y: axis_edges(self.lo[1], self.hi[1], self.ny, y_bands)?,
            z: self.z_top - self.thickness / 2.0,
        })
    }

    /// Whether a cell centre falls inside a hole.
    fn holed(&self, centre: [f64; 3]) -> bool {
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
    pub fn build_into(
        &self,
        geometry: &mut Geometry,
    ) -> Result<Vec<Vec<Option<NodeId>>>, PlaneError> {
        let mesh = self.mesh()?;
        let (nx, ny) = (mesh.nx(), mesh.ny());

        let mut centres: Vec<Vec<Option<NodeId>>> = (0..nx).map(|_| vec![None; ny]).collect();
        for (i, column) in centres.iter_mut().enumerate() {
            for (j, slot) in column.iter_mut().enumerate() {
                let position = mesh.centre(i, j);
                if self.holed(position) {
                    continue;
                }
                *slot =
                    Some(geometry.add_node(Node::new(position[0], position[1], position[2]))?);
            }
        }
        let mut bars = 0;
        for (i, column) in centres.iter().enumerate() {
            for (j, here) in column.iter().enumerate() {
                let Some(here) = *here else { continue };
                // An x-directed bar is as wide as the cell row's extent
                // across y; a y-directed bar as wide as the column's
                // extent across x.
                if i + 1 < nx {
                    if let Some(right) = centres[i + 1][j] {
                        geometry.add_segment(SegmentDef::new(
                            here,
                            right,
                            mesh.dy(j),
                            self.thickness,
                            self.sigma,
                        ))?;
                        bars += 1;
                    }
                }
                if j + 1 < ny {
                    if let Some(up) = column[j + 1] {
                        geometry.add_segment(SegmentDef::new(
                            here,
                            up,
                            mesh.dx(i),
                            self.thickness,
                            self.sigma,
                        ))?;
                        bars += 1;
                    }
                }
            }
        }
        debug_assert_eq!(
            bars,
            self.live_bars(&mesh),
            "bar count matches the independent count"
        );
        Ok(centres)
    }

    /// Independent count of live bars (for the debug assertion above and
    /// for tests).
    fn live_bars(&self, mesh: &PlaneMesh) -> usize {
        let live = |i: usize, j: usize| !self.holed(mesh.centre(i, j));
        let mut bars = 0;
        for i in 0..mesh.nx() {
            for j in 0..mesh.ny() {
                if live(i, j) && i + 1 < mesh.nx() && live(i + 1, j) {
                    bars += 1;
                }
                if live(i, j) && j + 1 < mesh.ny() && live(i, j + 1) {
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
    /// [`PlaneError::DegenerateExtent`] if the plane has no live cells,
    /// and anything [`GroundPlane::mesh`] reports.
    pub fn attach(
        &self,
        centres: &[Vec<Option<NodeId>>],
        point: [f64; 3],
    ) -> Result<NodeId, PlaneError> {
        let mesh = self.mesh()?;
        let mut best: Option<(f64, NodeId)> = None;
        for (i, column) in centres.iter().enumerate() {
            for (j, node) in column.iter().enumerate() {
                let Some(node) = *node else { continue };
                let centre = mesh.centre(i, j);
                let distance = (centre[0] - point[0]).hypot(centre[1] - point[1]);
                if best.is_none_or(|(current, _)| distance < current) {
                    best = Some((distance, node));
                }
            }
        }
        best.map(|(_, node)| node)
            .ok_or(PlaneError::DegenerateExtent {
                lo: self.lo,
                hi: self.hi,
            })
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
            contacts: Vec::new(),
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
        plane.holes.push(Hole {
            lo: [4.9e-3, 2.9e-3],
            hi: [5.1e-3, 3.1e-3],
        });
        let centres = plane.build_into(&mut geometry).unwrap();
        assert!(centres[2][1].is_none());
        assert_eq!(geometry.nodes().len(), 14);
        // 22 bars minus the 4 incident to the removed cell.
        assert_eq!(geometry.segment_count(), 18);
        // A hole exactly on a centre boundary does not remove the cell.
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        plane.holes.push(Hole {
            lo: [5e-3, 0.0],
            hi: [6e-3, 6e-3],
        });
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
        plane.holes.push(Hole {
            lo: [0.0, 0.0],
            hi: [2.0e-3, 2.0e-3],
        });
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
        let error = GroundPlane {
            nx: 0,
            ny: 3,
            ..test_plane()
        }
        .build_into(&mut geometry)
        .unwrap_err();
        assert_eq!(error, PlaneError::ZeroSubdivision { nx: 0, ny: 3 });
        let error = GroundPlane {
            hi: [0.0, 6e-3],
            ..test_plane()
        }
        .build_into(&mut geometry)
        .unwrap_err();
        assert!(matches!(error, PlaneError::DegenerateExtent { .. }));
    }

    /// The uniform mesh is exactly the edges the old `(dx, dy)` cell gave.
    #[test]
    fn uniform_mesh_edges_are_unchanged_without_contacts() {
        let mesh = test_plane().mesh().unwrap();
        assert_eq!((mesh.nx(), mesh.ny()), (5, 3));
        assert_eq!(mesh.bars(), 22);
        for i in 0..5 {
            assert!((mesh.x_edges()[i] - i as f64 * 2e-3).abs() < 1e-15);
            assert!((mesh.dx(i) - 2e-3).abs() < 1e-15);
        }
        assert_eq!(mesh.x_edges()[5], 10e-3);
        assert_eq!(mesh.y_edges()[3], 6e-3);
    }

    #[test]
    fn a_contact_region_refines_locally_and_decays_geometrically() {
        let plane = GroundPlane {
            contacts: vec![ContactRegion::centred([5e-3, 3e-3], 1e-3, 4, 2.0)],
            ..test_plane()
        };
        let mesh = plane.mesh().unwrap();
        // The region's own cells are uniform at 0.25 mm and land exactly
        // on its edges.
        let fine: Vec<usize> = (0..mesh.nx())
            .filter(|&i| (mesh.dx(i) - 0.25e-3).abs() < 1e-12)
            .collect();
        assert_eq!(fine.len(), 4, "four fine cells across the 1 mm region");
        assert!((mesh.x_edges()[fine[0]] - 4.5e-3).abs() < 1e-12);
        assert!((mesh.x_edges()[fine[3] + 1] - 5.5e-3).abs() < 1e-12);
        // Outside, cells grow by the ratio until they reach the 2 mm
        // background cell, and never exceed it.
        for i in 0..fine[0] {
            assert!(
                mesh.dx(i) <= 2e-3 + 1e-12,
                "cell {i} exceeds the background"
            );
        }
        let growth = mesh.dx(fine[0] - 1) / 0.25e-3;
        assert!(
            (1.0..=2.0 + 1e-9).contains(&growth),
            "the cell beside the region grows by at most the ratio ({growth})"
        );
        // Both axes are graded, and the edges still tile the footprint.
        assert!(mesh.ny() > 3, "the y axis is refined too ({})", mesh.ny());
        assert_eq!(mesh.x_edges()[0], 0.0);
        assert_eq!(*mesh.x_edges().last().unwrap(), 10e-3);
        assert_eq!(*mesh.y_edges().last().unwrap(), 6e-3);
        let span: f64 = (0..mesh.nx()).map(|i| mesh.dx(i)).sum();
        assert!((span - 10e-3).abs() < 1e-15, "the x cells tile the span");
    }

    /// Grading refines the contact far below the background cell while
    /// spending a small multiple of the background's cells.
    #[test]
    fn grading_costs_far_less_than_refining_everywhere() {
        let plane = GroundPlane {
            nx: 20,
            ny: 12,
            contacts: vec![ContactRegion::centred([5e-3, 3e-3], 0.5e-3, 4, 1.5)],
            ..test_plane()
        };
        let mesh = plane.mesh().unwrap();
        let finest = (0..mesh.nx()).map(|i| mesh.dx(i)).fold(f64::MAX, f64::min);
        assert!((finest - 0.125e-3).abs() < 1e-12, "fine cell {finest}");
        // A uniform mesh at the same resolution: 10 mm / 0.125 mm = 80 by
        // 6 mm / 0.125 mm = 48.
        let uniform = GroundPlane {
            nx: 80,
            ny: 48,
            ..test_plane()
        };
        let uniform = uniform.mesh().unwrap();
        assert!(
            mesh.bars() * 4 < uniform.bars(),
            "graded {} bars vs uniform {}",
            mesh.bars(),
            uniform.bars()
        );
    }

    /// A ratio of exactly 1 is legal and means "no decay": the axis is
    /// uniformly fine.
    #[test]
    fn a_unit_ratio_grades_into_a_uniform_fine_mesh() {
        let plane = GroundPlane {
            contacts: vec![ContactRegion::new([4e-3, 2e-3], [6e-3, 4e-3], [4, 4], 1.0)],
            ..test_plane()
        };
        let mesh = plane.mesh().unwrap();
        // 0.5 mm cells everywhere: 10 mm / 0.5 = 20 by 6 mm / 0.5 = 12.
        assert_eq!((mesh.nx(), mesh.ny()), (20, 12));
        for i in 0..mesh.nx() {
            assert!((mesh.dx(i) - 0.5e-3).abs() < 1e-12);
        }
    }

    #[test]
    fn contacts_compose_with_holes() {
        let plane = GroundPlane {
            contacts: vec![ContactRegion::centred([5e-3, 3e-3], 1e-3, 2, 2.0)],
            holes: vec![Hole {
                lo: [4.5e-3, 2.5e-3],
                hi: [5.5e-3, 3.5e-3],
            }],
            ..test_plane()
        };
        let mesh = plane.mesh().unwrap();
        let mut geometry = Geometry::new();
        let centres = plane.build_into(&mut geometry).unwrap();
        // Exactly the cells whose centre is inside the hole are gone —
        // including the refined ones the contact region created.
        let mut dead = 0;
        for (i, column) in centres.iter().enumerate() {
            for (j, slot) in column.iter().enumerate() {
                let centre = mesh.centre(i, j);
                let inside = 4.5e-3 < centre[0]
                    && centre[0] < 5.5e-3
                    && 2.5e-3 < centre[1]
                    && centre[1] < 3.5e-3;
                assert_eq!(slot.is_none(), inside, "cell ({i}, {j}) at {centre:?}");
                dead += usize::from(inside);
            }
        }
        assert!(dead >= 4, "the hole covers the refined cells ({dead})");
        assert_eq!(
            geometry.nodes().len(),
            mesh.nx() * mesh.ny() - dead,
            "holed cells carry no node"
        );
        assert_eq!(geometry.segment_count(), plane.live_bars(&mesh));
        assert!(geometry.segment_count() < mesh.bars());
    }

    #[test]
    fn a_contact_at_the_footprint_edge_is_clipped() {
        let plane = GroundPlane {
            // Half outside the plane on the left, and flush with y = 0.
            contacts: vec![ContactRegion::new(
                [-0.5e-3, -1e-3],
                [0.5e-3, 1e-3],
                [4, 8],
                2.0,
            )],
            ..test_plane()
        };
        let mesh = plane.mesh().unwrap();
        assert_eq!(mesh.x_edges()[0], 0.0);
        assert_eq!(mesh.y_edges()[0], 0.0);
        // The clipped band keeps the declared 0.25 mm x resolution, from
        // the footprint edge inward.
        assert!((mesh.dx(0) - 0.25e-3).abs() < 1e-12, "{}", mesh.dx(0));
        assert!((mesh.dx(1) - 0.25e-3).abs() < 1e-12);
        assert!((mesh.dy(0) - 0.25e-3).abs() < 1e-12, "{}", mesh.dy(0));
        // Cells still tile the footprint exactly.
        let span: f64 = (0..mesh.ny()).map(|j| mesh.dy(j)).sum();
        assert!((span - 6e-3).abs() < 1e-15);
    }

    #[test]
    fn two_contacts_grade_independently_and_merge_when_they_touch() {
        // Independent regions with their own ratios: both resolutions
        // appear, and the gap between them is coarse in the middle.
        let plane = GroundPlane {
            nx: 10,
            contacts: vec![
                ContactRegion::centred([2e-3, 3e-3], 0.4e-3, 4, 1.4),
                ContactRegion::centred([8e-3, 3e-3], 0.8e-3, 4, 2.0),
            ],
            ..test_plane()
        };
        let mesh = plane.mesh().unwrap();
        let extents: Vec<f64> = (0..mesh.nx()).map(|i| mesh.dx(i)).collect();
        assert!(extents.iter().any(|&d| (d - 0.1e-3).abs() < 1e-12));
        assert!(extents.iter().any(|&d| (d - 0.2e-3).abs() < 1e-12));
        // Between the two regions the mesh relaxes to the background cell.
        assert!(
            extents.iter().any(|&d| d > 0.9e-3),
            "the far field stays coarse: {extents:?}"
        );

        // Overlapping regions merge into one band at the finer cell.
        let plane = GroundPlane {
            contacts: vec![
                ContactRegion::new([4e-3, 2e-3], [5e-3, 4e-3], [4, 4], 2.0),
                ContactRegion::new([4.5e-3, 2e-3], [5.5e-3, 4e-3], [10, 4], 2.0),
            ],
            ..test_plane()
        };
        let mesh = plane.mesh().unwrap();
        let fine = (0..mesh.nx())
            .filter(|&i| (mesh.dx(i) - 0.1e-3).abs() < 1e-12)
            .count();
        assert_eq!(fine, 15, "the merged 1.5 mm band is cut at 0.1 mm");
    }

    #[test]
    fn errors_for_unusable_contact_regions() {
        let bad = |contact: ContactRegion| {
            GroundPlane {
                contacts: vec![contact],
                ..test_plane()
            }
            .mesh()
            .unwrap_err()
        };
        assert_eq!(
            bad(ContactRegion::centred([5e-3, 3e-3], 1e-3, 4, 0.5)),
            PlaneError::ContactRatio { ratio: 0.5 }
        );
        assert_eq!(
            bad(ContactRegion::new([4e-3, 2e-3], [6e-3, 4e-3], [0, 4], 2.0)),
            PlaneError::ContactZeroCells { cells: [0, 4] }
        );
        assert!(matches!(
            bad(ContactRegion::new([6e-3, 2e-3], [4e-3, 4e-3], [4, 4], 2.0)),
            PlaneError::ContactExtent { .. }
        ));
        assert!(matches!(
            bad(ContactRegion::centred([20e-3, 3e-3], 1e-3, 4, 2.0)),
            PlaneError::ContactOutsideFootprint { .. }
        ));
        // A contact finer than the per-axis cell limit is refused rather
        // than filling memory.
        assert_eq!(
            bad(ContactRegion::centred([5e-3, 3e-3], 1e-9, 100, 1.0)),
            PlaneError::MeshTooLarge {
                limit: MAX_AXIS_CELLS
            }
        );
        // ...and so is a region whose *own* cells exceed the limit,
        // counted before the band is laid out rather than after (a cell
        // count this large would otherwise allocate the axis first). The
        // `usize::MAX` case makes the fine cell underflow to zero, which
        // is why the count, not the extent, is what is checked.
        for cells in [MAX_AXIS_CELLS + 1, usize::MAX] {
            assert_eq!(
                bad(ContactRegion::new(
                    [4e-3, 2e-3],
                    [6e-3, 4e-3],
                    [cells, 4],
                    1.5
                )),
                PlaneError::MeshTooLarge {
                    limit: MAX_AXIS_CELLS
                },
                "{cells} cells across the region"
            );
        }
    }

    /// Many individually-affordable regions must not add up to an
    /// unaffordable axis either.
    #[test]
    fn many_contacts_hit_the_axis_cell_limit_together() {
        let contacts: Vec<ContactRegion> = (0..40)
            .map(|k| {
                let x = 0.1e-3 + k as f64 * 0.24e-3;
                ContactRegion::new([x, 2e-3], [x + 0.1e-3, 4e-3], [5_000, 4], 1.5)
            })
            .collect();
        assert_eq!(
            GroundPlane {
                contacts,
                ..test_plane()
            }
            .mesh()
            .unwrap_err(),
            PlaneError::MeshTooLarge {
                limit: MAX_AXIS_CELLS
            }
        );
    }
}
