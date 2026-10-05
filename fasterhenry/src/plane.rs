//! Ground planes: thick rectangular sheets discretized into a mesh of
//! bars, with holes (rectangle, point or circle) and locally refined
//! contact regions — the FastHenry `G` feature, on the same
//! segment/kernel/mesh machinery everything else uses.
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
//! A hole removes the cells it covers together with their incident bars —
//! see [`Hole`] for each shape's exact cell-removal rule. This is a PEEC
//! discretization in its own right (not a transcription of FastHenry's
//! particular panel mesh — see the clean-room note in the repository
//! README): convergence to the continuum plane goes as the grid is
//! refined.
//!
//! # Graded contact regions
//!
//! A via landing pulls the return current into a small patch of the
//! plane, and that patch is where the mesh has to be fine: a uniform
//! cell-centre grid converges only like `1/n`, so resolving a landing by
//! refining everywhere is ruinous. A [`ContactRegion`] refines locally
//! instead — inside the region the mesh is uniform at the fine cell
//! `h = w / cells`, and outside it each cell is `ratio` times its
//! neighbour until it reaches the background cell `c`. The ratio is
//! itself per axis, so a region that is refined harder across x than
//! across y decays at its own rate on each.
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
//! ## A band that is not a refinement
//!
//! One band takes no part in that merge: one spanning the **whole axis** at
//! the plane's own cell count for it. Such a band *is* the background mesh
//! on that axis — its cells are the background cells, and the only edges it
//! pins are the footprint's own, which are edges regardless — so it adds no
//! resolution and is dropped before the bands are merged. It has to be:
//! spanning the axis, it overlaps every other band on it, and merging keeps
//! the finer cell across the union of the two extents, so one such band
//! would refine the *entire* axis to the finest cell anywhere on it. That is
//! a cost nothing in the request asked for (issue #124). A deck's `contact
//! point` / `contact line` emits exactly this band for an axis whose
//! requested cell the background already meets, so that axis's edges are
//! the ones it has without the clause (issue #116).
//!
//! A [`ContactRegion`] cut at the background cell over only **part** of an
//! axis is *not* dropped: it still cuts its own extent into its own cells,
//! and so still pins its `lo` and `hi` as cell edges — a caller naming a
//! coordinate gets an edge there even where the resolution is already met,
//! and it still merges with a neighbour as any other band does. Only the
//! whole-axis case is a no-op, and only the no-op is dropped.
//!
//! ## Cell count
//!
//! Per axis, for a span `W` with background count `n` (coarse cell
//! `c = W/n`) and one region of width `w` cut into `k` cells
//! (`h = w/k`) at that axis's ratio `r`: the region contributes `k`
//! cells, and each gap of length `g` beside it takes the smallest `m`
//! cells with
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
//! # Equipotential contacts
//!
//! A landing that is a *pad* rather than a point — a wide via, a bond pad,
//! the footprint of a connector — shorts a whole patch of the plane to one
//! potential. An [`Equipotential`] rectangle says exactly that: every live
//! cell centre inside it shares a single node, so the patch has no internal
//! drop and current enters it across its whole boundary instead of through
//! one cell. This is what a deck's *named contact area* needs, and it is a
//! different thing from a [`ContactRegion`], which only grades the mesh.
//!
//! The tie changes the mesh, not just a label, so its consequences are
//! stated rather than left to be inferred:
//!
//! * the bars **inside** the patch disappear — both of their ends are the
//!   same node, and a perfect conductor carries no drop across itself;
//! * the bars that **cross the patch's boundary** run from their own cell
//!   centre to the tie node, which sits at the mean of the cell centres it
//!   ties. The model is therefore exact for a patch covering one cell; for
//!   a larger patch each entering bar reaches that one point through metal
//!   the mesh still treats as ordinary plane rather than as the perfect
//!   conductor the patch is, which over-states the patch's own resistance
//!   rather than under-stating it — and, unlike landing on a single cell
//!   centre, the over-statement does not grow as the mesh is refined;
//! * two rectangles that tie a cell in common are **one** equipotential
//!   (two overlapping perfect conductors are one conductor), so overlapping
//!   regions merge rather than needing a tie-break;
//! * a rectangle catching no live cell centre ties nothing, and
//!   [`GroundPlane::equipotential_node`] reports `None` for it.
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
//! footprint), never silent elsewhere. An endpoint landing inside an
//! [`Equipotential`] rectangle snaps to a cell of that patch, and so to the
//! patch's one tie node, with no further machinery; a caller that wants the
//! tie node itself — a deck naming a *contact area* rather than a point —
//! asks [`GroundPlane::equipotential_node`] for it.

use thiserror::Error;

use crate::geometry::{Geometry, GeometryError, Node, NodeId, SegmentDef};

/// A hole cut into a plane's mesh: the region whose covered cells are
/// removed together with their incident bars.
///
/// Each variant states its own cell-removal rule — and, where the shape can
/// produce one, the rule at a tie — rather than sharing one geometric test:
/// a rectangle, a point and a circle are not comparable shapes, so forcing
/// one rule onto all three would either silently misrepresent one of them
/// or need an undocumented guess (see [`GroundPlane::build_into`], which
/// applies every hole in [`GroundPlane::holes`] cell by cell).
///
/// `#[non_exhaustive]`: this is a list of *shapes*, and the list is not
/// claimed to be closed, so adding one should not be a breaking change the
/// way this enum's own introduction was (issue #98 widened this from a
/// rectangle-only struct).
///
/// # Arbitrary removal rules
///
/// There is deliberately **no** predicate or callback variant here (issue
/// #99; the decision and its reasoning are recorded in
/// `docs/fasthenry-compat.md` § *`hole user1`…`user7`*). A hole whose rule
/// is "whatever this program decides, cell by cell" needs no new variant,
/// because [`GroundPlane::mesh`] does not depend on
/// [`GroundPlane::holes`]: mesh the plane first, apply your own rule to the
/// cell centres the mesh reports, and cut each selected cell with a
/// [`Hole::Point`] at its own centre. A centre lies strictly inside its own
/// cell, so each such point removes exactly that one cell and no other —
/// the composition is exact, not an approximation.
///
/// ```
/// use fasterhenry::geometry::Geometry;
/// use fasterhenry::plane::{GroundPlane, Hole};
///
/// let mut plane = GroundPlane {
///     lo: [0.0, 0.0],
///     hi: [10e-3, 6e-3],
///     z_top: 0.0,
///     thickness: 35e-6,
///     nx: 5,
///     ny: 3,
///     sigma: 5.8e7,
///     ..GroundPlane::default()
/// };
///
/// // Any rule at all over a cell centre — here a diagonal cut that no
/// // rectangle, point or circle describes.
/// let cut = |centre: [f64; 2]| centre[1] > centre[0] / 2.0;
///
/// let mesh = plane.mesh()?;
/// for i in 0..mesh.nx() {
///     for j in 0..mesh.ny() {
///         let centre = mesh.centre(i, j);
///         if cut([centre[0], centre[1]]) {
///             plane.holes.push(Hole::Point {
///                 at: [centre[0], centre[1]],
///             });
///         }
///     }
/// }
///
/// let mut geometry = Geometry::new();
/// let live = plane.build_into(&mut geometry)?;
/// assert!(live[0][2].is_none()); // centre (1 mm, 5 mm): above the cut
/// assert!(live[4][0].is_some()); // centre (9 mm, 1 mm): below it
/// # Ok::<(), fasterhenry::plane::PlaneError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Hole {
    /// An axis-aligned rectangle: removes every cell whose **centre**
    /// lies strictly inside `lo .. hi` (open on both edges) — a centre
    /// exactly on the rectangle's boundary is not removed. This is the
    /// original rectangle rule, unchanged from before this type became an
    /// enum.
    Rect {
        /// Inclusive lower corner `(x, y)`.
        lo: [f64; 2],
        /// Exclusive upper corner `(x, y)`; see the strict-inequality note
        /// above.
        hi: [f64; 2],
    },
    /// A single point: removes every cell whose own extent — edges
    /// included — contains it. A point strictly inside exactly one cell
    /// removes that cell alone; a point exactly on a shared edge (or
    /// corner) between cells is the documented tie, and removes every
    /// cell touching that edge or corner rather than guessing a single
    /// winner among them.
    Point {
        /// The point `(x, y)`.
        at: [f64; 2],
    },
    /// A circle: removes every cell whose **centre** lies at or inside
    /// `radius` of `centre` — a centre exactly on the circle is removed
    /// (closed boundary; there is no legacy rectangle-style behaviour to
    /// match here, so the boundary is defined inclusive).
    Circle {
        /// Centre `(x, y)`.
        centre: [f64; 2],
        /// Radius, metres. A negative value is not rejected by this type
        /// (a distance is never negative, so `centre`'s test is simply
        /// always false and the hole removes nothing) — the deck reader
        /// validates `radius >= 0` itself, since a negative radius on a
        /// deck line is a mistake to report, not a shape to build.
        radius: f64,
    },
}

impl Hole {
    /// Whether this hole removes a cell, given the cell's own centre and
    /// its extent `(lo, hi)` in the plane's xy coordinates. `bounds` only
    /// matters for [`Hole::Point`]; see each variant's doc for its test.
    fn removes(&self, centre: [f64; 3], bounds: ([f64; 2], [f64; 2])) -> bool {
        match *self {
            Hole::Rect { lo, hi } => {
                lo[0] < centre[0] && centre[0] < hi[0] && lo[1] < centre[1] && centre[1] < hi[1]
            }
            Hole::Point { at } => {
                let (lo, hi) = bounds;
                lo[0] <= at[0] && at[0] <= hi[0] && lo[1] <= at[1] && at[1] <= hi[1]
            }
            Hole::Circle { centre: c, radius } => {
                (centre[0] - c[0]).hypot(centre[1] - c[1]) <= radius
            }
        }
    }
}

/// A rectangular contact region: the patch of plane under a via landing,
/// meshed finely and decaying back to the background cell outside.
///
/// Inside `[lo, hi]` the mesh is uniform with `cells` cells per axis (fine
/// cell `h = (hi − lo) / cells`); outside, cells grow by that axis's
/// `ratio` each until they reach the background cell. See the [module
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
    /// Geometric growth ratio per cell outside the region, per axis
    /// `[x, y]` (each `>= 1`; `1` is no decay at all on that axis, i.e. a
    /// uniformly fine axis). [`ContactRegion::new`] sets both alike;
    /// [`ContactRegion::graded_per_axis`] sets them apart, which is what a
    /// region refined harder across one axis than the other needs.
    pub ratio: [f64; 2],
}

impl ContactRegion {
    /// A region spanning `lo … hi`, `cells` fine cells per axis, decaying
    /// outward by `ratio` on both axes alike.
    #[must_use]
    pub fn new(lo: [f64; 2], hi: [f64; 2], cells: [usize; 2], ratio: f64) -> Self {
        Self::graded_per_axis(lo, hi, cells, [ratio, ratio])
    }

    /// A region spanning `lo … hi`, `cells` fine cells per axis, decaying
    /// outward by a ratio chosen per axis (`[x, y]`).
    ///
    /// The anisotropic case [`ContactRegion::new`]'s single ratio cannot
    /// express: a region whose refinement differs between the axes decays
    /// back to the background cell at a different rate on each.
    #[must_use]
    pub fn graded_per_axis(lo: [f64; 2], hi: [f64; 2], cells: [usize; 2], ratio: [f64; 2]) -> Self {
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

/// A rectangular patch of plane tied to a single node: every live cell
/// centre inside it shares one node, so the patch is an equipotential.
///
/// A [`ContactRegion`] grades the mesh under a landing; this ties the metal
/// together. The two are independent and compose: a pad that is both
/// resolved finely and shorted is one of each on the same rectangle.
///
/// Membership is by cell **centre**, with the rectangle's boundary
/// **included** — a centre exactly on an edge is tied. (There is no legacy
/// rectangle behaviour to match here, as there is for [`Hole::Rect`]'s open
/// test, and a contact names a patch of metal: excluding a centre on the
/// edge would quietly shrink the patch the deck asked for.) See the [module
/// documentation](self#equipotential-contacts) for what the tie does to the
/// mesh, and [`GroundPlane::equipotential_node`] for the node it produces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Equipotential {
    /// Lower corner `(x, y)` in plane coordinates, metres.
    pub lo: [f64; 2],
    /// Upper corner `(x, y)`, metres.
    pub hi: [f64; 2],
}

impl Equipotential {
    /// The patch spanning `lo … hi`.
    #[must_use]
    pub fn new(lo: [f64; 2], hi: [f64; 2]) -> Self {
        Self { lo, hi }
    }

    /// The patch of full widths `widths` about `centre` — the centre-and-
    /// widths spelling a deck's contact clauses use.
    #[must_use]
    pub fn centred(centre: [f64; 2], widths: [f64; 2]) -> Self {
        Self {
            lo: [centre[0] - widths[0] / 2.0, centre[1] - widths[1] / 2.0],
            hi: [centre[0] + widths[0] / 2.0, centre[1] + widths[1] / 2.0],
        }
    }

    /// Whether this patch ties the cell with the given centre (boundary
    /// included; see the type's own documentation).
    fn ties(&self, centre: [f64; 3]) -> bool {
        self.lo[0] <= centre[0]
            && centre[0] <= self.hi[0]
            && self.lo[1] <= centre[1]
            && centre[1] <= self.hi[1]
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
    /// Holes cut into the mesh (x/y in plane coordinates); see [`Hole`] for
    /// each shape's removal rule, and for the way an arbitrary per-cell rule
    /// composes from [`GroundPlane::mesh`] and [`Hole::Point`].
    pub holes: Vec<Hole>,
    /// Locally refined contact regions (x/y in plane coordinates).
    pub contacts: Vec<ContactRegion>,
    /// Patches tied to one node each (x/y in plane coordinates); see
    /// [`Equipotential`] and the [module
    /// documentation](self#equipotential-contacts).
    pub equipotentials: Vec<Equipotential>,
}

/// Why a ground plane could not be built.
#[derive(Clone, Debug, PartialEq, Error)]
#[non_exhaustive]
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

    /// The lower and upper corners `(x, y)` of cell `(i, j)`'s own extent.
    ///
    /// # Panics
    ///
    /// If `i >= self.nx()` or `j >= self.ny()`.
    #[must_use]
    pub fn cell_bounds(&self, i: usize, j: usize) -> ([f64; 2], [f64; 2]) {
        ([self.x[i], self.y[j]], [self.x[i + 1], self.y[j + 1]])
    }

    /// Bars the mesh would carry with every cell live:
    /// `2·nx·ny − nx − ny`.
    #[must_use]
    pub fn bars(&self) -> usize {
        2 * self.nx() * self.ny() - self.nx() - self.ny()
    }
}

/// The overlap, in metres, below which a contact region's clipped extent
/// along one plane axis `lo … hi` is a *touch* rather than a region
/// (issue #134). Two independent tolerances, whichever is larger:
///
/// - a **shape-relative** term, `1e-9` of the axis's span `|hi − lo|`, so
///   the threshold follows the plane's own size in any unit; and
/// - a **coordinate-rounding** term, `32·f64::EPSILON` times the axis's
///   largest coordinate magnitude `max(|lo|, |hi|)` — a few ULPs of the
///   coordinates themselves, which is all the noise a translated plane
///   picks up (the same pattern as the filament grader's extent floor).
///
/// Keeping the two apart matters for a small plane far from the origin:
/// scaling the coordinate magnitude by the shape tolerance (`1e-9·|lo|`)
/// would, for a 10 mm axis at `1e9` m, demand a ~1 m overlap — wider than
/// the plane — while the rounding term there is ~7 µm. Either way the
/// slack stays far below any refinement worth asking for.
///
/// [`GroundPlane::mesh`] rejects a contact whose overlap with the
/// footprint is no wider than this on either axis as
/// [`PlaneError::ContactOutsideFootprint`], the same as an exact touch.
#[must_use]
pub fn contact_slack(lo: f64, hi: f64) -> f64 {
    let shape = 1e-9 * (hi - lo).abs();
    let rounding = 32.0 * f64::EPSILON * lo.abs().max(hi.abs());
    shape.max(rounding)
}

/// The fewest uniform cells across `span` whose extent is no larger than
/// `fine` — `ceil(span / fine)`, at least one.
///
/// A span that is an exact multiple of `fine` must not gain a spurious
/// extra cell from a last-bit rounding of the division, so a quotient
/// within a relative `1e-9` of an integer counts as that integer. Rounding
/// to the *nearest* count instead (as band layout once did) could cut a
/// merged or edge-widened band into cells up to twice the requested fine
/// cell, breaking the "keeps the finest cell" guarantee the refinement
/// shapes rely on.
fn cells_no_coarser_than(span: f64, fine: f64) -> usize {
    let quotient = span / fine;
    let rounded = quotient.round();
    let cells = if (quotient - rounded).abs() <= 1e-9 * rounded {
        rounded
    } else {
        quotient.ceil()
    };
    // `as` saturates: an infinite quotient (a fine cell that underflowed
    // to zero) becomes `usize::MAX`, which the per-axis limit then refuses.
    (cells as usize).max(1)
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
    // A band spanning the whole axis at the axis's own cell count *is* the
    // background mesh: its cells are the background cells and the only edges
    // it pins are the footprint's own, which are edges regardless. It adds no
    // resolution, so it takes no part in the merge below — where, spanning the
    // axis, it would overlap every other band and widen the axis's finest cell
    // across the whole of it (issue #124). A band at the background cell over
    // only *part* of an axis is kept: see the module documentation.
    bands.retain(|band| !(band.start <= lo && band.end >= hi && band.cells == background));
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
                last.cells = cells_no_coarser_than(last.end - last.start, fine);
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

/// The equipotential classes of a meshed plane: which [`Equipotential`]
/// each live cell belongs to once regions sharing a cell have merged, and
/// where each class's one tie node sits.
#[derive(Clone, Debug)]
struct Ties {
    /// The representative region of each declared region.
    root: Vec<usize>,
    /// The representative region of each cell, `[i][j]`; `None` where the
    /// cell is holed or no region ties it.
    cell: Vec<Vec<Option<usize>>>,
    /// The tie node's position, per region; `Some` only for a
    /// representative that ties at least one live cell.
    node_at: Vec<Option<[f64; 3]>>,
}

impl Ties {
    /// Whether two cells are tied into the same equipotential.
    fn joined(&self, a: (usize, usize), b: (usize, usize)) -> bool {
        match (self.cell[a.0][a.1], self.cell[b.0][b.1]) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    }
}

/// Union-find over region indices, with path halving.
fn find(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

impl GroundPlane {
    /// The plane's cell layout — uniform `nx × ny`, refined around every
    /// [`ContactRegion`].
    ///
    /// The layout does **not** depend on [`GroundPlane::holes`]: holes
    /// remove cells from this mesh, they never move its edges. A caller may
    /// therefore mesh a plane, decide from the cell centres which cells it
    /// wants gone, and only then push the holes that remove them — see
    /// [`Hole`] § *Arbitrary removal rules*.
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
            if let Some(&ratio) = contact
                .ratio
                .iter()
                .find(|ratio| !(ratio.is_finite() && **ratio >= 1.0))
            {
                return Err(PlaneError::ContactRatio { ratio });
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
                // An overlap no wider than the footprint's own rounding
                // noise is a touch, not a region: a band that thin would
                // lay down edges closer than a float can tell apart (a
                // zero-length bar at assembly), so it is rejected exactly
                // as an exact touch (`end == start`) is (issue #134).
                if end - start <= contact_slack(self.lo[axis], self.hi[axis]) {
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
                    cells: cells_no_coarser_than(end - start, fine),
                    ratio: contact.ratio[axis],
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

    /// Whether a cell is removed by any hole. `bounds` is the cell's own
    /// extent, `(lo, hi)`; see [`Hole`] for the per-shape removal rule.
    fn holed(&self, centre: [f64; 3], bounds: ([f64; 2], [f64; 2])) -> bool {
        self.holes.iter().any(|hole| hole.removes(centre, bounds))
    }

    /// Which [`Equipotential`] ties each live cell of `mesh`, and where each
    /// class's node sits.
    ///
    /// Regions that tie a live cell in common are merged into one class —
    /// two overlapping perfect conductors are one conductor — so no
    /// tie-break between overlapping rectangles is needed. A class's node
    /// sits at the mean of the cell centres it ties.
    fn ties(&self, mesh: &PlaneMesh) -> Ties {
        let (nx, ny) = (mesh.nx(), mesh.ny());
        let mut root: Vec<usize> = (0..self.equipotentials.len()).collect();
        let mut cell: Vec<Vec<Option<usize>>> = (0..nx).map(|_| vec![None; ny]).collect();
        if self.equipotentials.is_empty() {
            return Ties {
                root,
                cell,
                node_at: Vec::new(),
            };
        }
        // Pass 1: every region tying a given live cell joins that cell's
        // class, so regions overlapping on the mesh merge.
        for (i, column) in cell.iter_mut().enumerate() {
            for (j, slot) in column.iter_mut().enumerate() {
                let centre = mesh.centre(i, j);
                if self.holed(centre, mesh.cell_bounds(i, j)) {
                    continue;
                }
                for (index, region) in self.equipotentials.iter().enumerate() {
                    if !region.ties(centre) {
                        continue;
                    }
                    let index = find(&mut root, index);
                    match *slot {
                        None => *slot = Some(index),
                        Some(first) => {
                            let first = find(&mut root, first);
                            root[index] = first;
                            *slot = Some(first);
                        }
                    }
                }
            }
        }
        // Pass 2: resolve every cell to its class's final representative,
        // accumulating each class's tie position as it goes.
        let mut sum = vec![[0.0f64; 3]; self.equipotentials.len()];
        let mut tied = vec![0usize; self.equipotentials.len()];
        for (i, column) in cell.iter_mut().enumerate() {
            for (j, slot) in column.iter_mut().enumerate() {
                let Some(index) = *slot else { continue };
                let index = find(&mut root, index);
                *slot = Some(index);
                let centre = mesh.centre(i, j);
                for axis in 0..3 {
                    sum[index][axis] += centre[axis];
                }
                tied[index] += 1;
            }
        }
        let node_at = sum
            .iter()
            .zip(&tied)
            .map(|(sum, &tied)| (tied > 0).then(|| sum.map(|total| total / tied as f64)))
            .collect();
        Ties {
            root,
            cell,
            node_at,
        }
    }

    /// The node every live cell of equipotential region `index` shares, as
    /// [`GroundPlane::build_into`] built it into `centres` — the node a deck
    /// naming a *contact area* attaches to. `None` when the rectangle
    /// catches no live cell centre, and so ties nothing.
    ///
    /// Regions merged with `index` (see [`Equipotential`]) report the same
    /// node, whichever of them is asked.
    ///
    /// # Errors
    ///
    /// Whatever [`GroundPlane::mesh`] reports.
    ///
    /// # Panics
    ///
    /// If `index >= self.equipotentials.len()`.
    pub fn equipotential_node(
        &self,
        centres: &[Vec<Option<NodeId>>],
        index: usize,
    ) -> Result<Option<NodeId>, PlaneError> {
        assert!(
            index < self.equipotentials.len(),
            "equipotential {index} is not one of this plane's {}",
            self.equipotentials.len()
        );
        let mesh = self.mesh()?;
        let mut ties = self.ties(&mesh);
        let root = find(&mut ties.root, index);
        for (i, column) in ties.cell.iter().enumerate() {
            for (j, class) in column.iter().enumerate() {
                if *class == Some(root) {
                    return Ok(centres[i][j]);
                }
            }
        }
        Ok(None)
    }

    /// Builds the plane's mesh into `geometry`, returning the node id of
    /// each live cell centre, indexed `[i][j]` (`None` where a hole
    /// removed the cell).
    ///
    /// Cells an [`Equipotential`] ties **share** one node id, so the same id
    /// appears at every `[i][j]` of that patch and the bars that would have
    /// run inside it are not built; see the [module
    /// documentation](self#equipotential-contacts).
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
        let ties = self.ties(&mesh);
        // One node per equipotential class, created at the first cell it
        // ties so that a plane without equipotentials numbers its nodes
        // exactly as it always has.
        let mut tie_nodes: Vec<Option<NodeId>> = vec![None; self.equipotentials.len()];

        let mut centres: Vec<Vec<Option<NodeId>>> = (0..nx).map(|_| vec![None; ny]).collect();
        for (i, column) in centres.iter_mut().enumerate() {
            for (j, slot) in column.iter_mut().enumerate() {
                let position = mesh.centre(i, j);
                if self.holed(position, mesh.cell_bounds(i, j)) {
                    continue;
                }
                *slot = Some(match ties.cell[i][j] {
                    Some(class) => match tie_nodes[class] {
                        Some(node) => node,
                        None => {
                            let at = ties.node_at[class].expect("a tied class ties a live cell");
                            let node = geometry.add_node(Node::new(at[0], at[1], at[2]))?;
                            tie_nodes[class] = Some(node);
                            node
                        }
                    },
                    None => geometry.add_node(Node::new(position[0], position[1], position[2]))?,
                });
            }
        }
        let mut bars = 0;
        for (i, column) in centres.iter().enumerate() {
            for (j, here) in column.iter().enumerate() {
                let Some(here) = *here else { continue };
                // An x-directed bar is as wide as the cell row's extent
                // across y; a y-directed bar as wide as the column's
                // extent across x. A bar whose two cells are tied into one
                // equipotential is not built at all: its ends are the same
                // node, and a perfect conductor carries no drop.
                if i + 1 < nx {
                    if let Some(right) = centres[i + 1][j].filter(|&right| right != here) {
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
                    if let Some(up) = column[j + 1].filter(|&up| up != here) {
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
    /// for tests): adjacent live cells that are not tied into the same
    /// equipotential.
    fn live_bars(&self, mesh: &PlaneMesh) -> usize {
        let ties = self.ties(mesh);
        let live = |i: usize, j: usize| !self.holed(mesh.centre(i, j), mesh.cell_bounds(i, j));
        let mut bars = 0;
        for i in 0..mesh.nx() {
            for j in 0..mesh.ny() {
                if !live(i, j) {
                    continue;
                }
                if i + 1 < mesh.nx() && live(i + 1, j) && !ties.joined((i, j), (i + 1, j)) {
                    bars += 1;
                }
                if j + 1 < mesh.ny() && live(i, j + 1) && !ties.joined((i, j), (i, j + 1)) {
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
            equipotentials: Vec::new(),
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
        plane.holes.push(Hole::Rect {
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
        plane.holes.push(Hole::Rect {
            lo: [5e-3, 0.0],
            hi: [6e-3, 6e-3],
        });
        plane.build_into(&mut geometry).unwrap();
        assert_eq!(geometry.nodes().len(), 15);
    }

    #[test]
    fn point_hole_removes_the_cell_containing_it() {
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        // Cell (2, 1) spans x in [4mm, 6mm], y in [2mm, 4mm]; a point well
        // inside it removes only that cell.
        plane.holes.push(Hole::Point {
            at: [5.5e-3, 3.5e-3],
        });
        let centres = plane.build_into(&mut geometry).unwrap();
        assert!(centres[2][1].is_none());
        assert_eq!(geometry.nodes().len(), 14);
        assert_eq!(geometry.segment_count(), 18);

        // A point exactly on the shared edge between cells (1,1) and (2,1)
        // (x = 4mm) is the documented tie: both cells are removed.
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        plane.holes.push(Hole::Point { at: [4e-3, 3.5e-3] });
        let centres = plane.build_into(&mut geometry).unwrap();
        assert!(centres[1][1].is_none());
        assert!(centres[2][1].is_none());
        assert_eq!(geometry.nodes().len(), 13);

        // A point outside the plane's footprint removes nothing.
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        plane.holes.push(Hole::Point { at: [50e-3, 3e-3] });
        let centres = plane.build_into(&mut geometry).unwrap();
        assert!(centres.iter().flatten().all(Option::is_some));
    }

    #[test]
    fn circle_hole_removes_cells_whose_centre_falls_inside() {
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        // Cell centres sit on a 2 mm grid — (1,1), (3,1), (5,1), (7,1),
        // (9,1) mm on the bottom row (j=0), (1,3)…(9,3) on the row above
        // (j=1). A circle of radius 2.5 mm around (5, 1) mm reaches its
        // three grid neighbours 2 mm away — (3,1) and (7,1) in x, (5,3) in
        // y — but not the diagonal ones (2.83 mm away) or the row's far
        // ends (4 mm away).
        plane.holes.push(Hole::Circle {
            centre: [5e-3, 1e-3],
            radius: 2.5e-3,
        });
        let centres = plane.build_into(&mut geometry).unwrap();
        for (i, j) in [(1, 0), (2, 0), (3, 0), (2, 1)] {
            assert!(centres[i][j].is_none(), "cell ({i}, {j}) should be removed");
        }
        for (i, j) in [(0, 0), (4, 0), (0, 1), (1, 1), (3, 1), (4, 1)] {
            assert!(centres[i][j].is_some(), "cell ({i}, {j}) should survive");
        }
        assert_eq!(geometry.nodes().len(), 11);

        // A centre exactly on the circle's boundary is removed (closed).
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        plane.holes.push(Hole::Circle {
            centre: [5e-3, 1e-3],
            radius: 2e-3,
        });
        let centres = plane.build_into(&mut geometry).unwrap();
        assert!(centres[2][0].is_none()); // centre (5, 1) itself
        assert!(centres[1][0].is_none()); // (3, 1): distance exactly 2mm
        assert!(centres[3][0].is_none()); // (7, 1): distance exactly 2mm
        assert!(centres[2][1].is_none()); // (5, 3): distance exactly 2mm

        // A circle large enough to cover the whole plane removes every
        // cell.
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        plane.holes.push(Hole::Circle {
            centre: [5e-3, 3e-3],
            radius: 100e-3,
        });
        let centres = plane.build_into(&mut geometry).unwrap();
        assert!(centres.iter().flatten().all(Option::is_none));
        assert_eq!(geometry.nodes().len(), 0);
        assert_eq!(geometry.segment_count(), 0);
    }

    /// The escape hatch issue #99 chose over a predicate variant: because
    /// [`GroundPlane::mesh`] ignores [`GroundPlane::holes`], an arbitrary
    /// per-cell rule is exactly expressible as one [`Hole::Point`] per
    /// selected cell centre.
    #[test]
    fn an_arbitrary_rule_composes_from_mesh_and_point_holes() {
        // Cuts the cells an arbitrary rule selects, and reports which
        // survived, `[i][j]`.
        let build = |rule: &dyn Fn(usize, usize) -> bool| {
            let mut plane = test_plane();
            let mesh = plane.mesh().unwrap();
            for i in 0..mesh.nx() {
                for j in 0..mesh.ny() {
                    if rule(i, j) {
                        let centre = mesh.centre(i, j);
                        plane.holes.push(Hole::Point {
                            at: [centre[0], centre[1]],
                        });
                    }
                }
            }
            let mut geometry = Geometry::new();
            let live = plane.build_into(&mut geometry).unwrap();
            (live, geometry.nodes().len())
        };

        // A checkerboard: no rectangle, point or circle describes it, and
        // each point removes exactly its own cell — never a neighbour.
        let checker = |i: usize, j: usize| (i + j) % 2 == 0;
        let (live, nodes) = build(&checker);
        for (i, column) in live.iter().enumerate() {
            for (j, cell) in column.iter().enumerate() {
                assert_eq!(
                    cell.is_none(),
                    checker(i, j),
                    "cell ({i}, {j}) follows the rule"
                );
            }
        }
        assert_eq!(nodes, 7); // 15 cells, 8 on the cut colour

        // An always-true rule removes every cell; an always-false one
        // removes none and leaves the unholed mesh untouched.
        let (live, nodes) = build(&|_, _| true);
        assert!(live.iter().flatten().all(Option::is_none));
        assert_eq!(nodes, 0);
        let (live, nodes) = build(&|_, _| false);
        assert!(live.iter().flatten().all(Option::is_some));
        assert_eq!(nodes, 15);
    }

    /// The same composition on a *graded* plane: the rule is applied to the
    /// mesh the plane will actually be built with, so a contact region's
    /// refinement does not change which cells the rule removes.
    #[test]
    fn the_arbitrary_rule_composition_holds_on_a_graded_mesh() {
        let mut plane = test_plane();
        plane
            .contacts
            .push(ContactRegion::centred([5e-3, 3e-3], 1e-3, 2, 2.0));
        let mesh = plane.mesh().unwrap();
        let (nx, ny) = (mesh.nx(), mesh.ny());
        assert!(nx > 5 && ny > 3, "the contact region refined the mesh");

        // Everything left of the plane's midline, however the mesh grades.
        let cut = |centre: [f64; 2]| centre[0] < 5e-3;
        let mut expected = 0;
        for i in 0..nx {
            for j in 0..ny {
                let centre = mesh.centre(i, j);
                if cut([centre[0], centre[1]]) {
                    plane.holes.push(Hole::Point {
                        at: [centre[0], centre[1]],
                    });
                    expected += 1;
                }
            }
        }
        assert!(expected > 0 && expected < nx * ny, "a genuine partition");

        let mut geometry = Geometry::new();
        let live = plane.build_into(&mut geometry).unwrap();
        for (i, column) in live.iter().enumerate() {
            for (j, cell) in column.iter().enumerate() {
                let centre = mesh.centre(i, j);
                assert_eq!(
                    cell.is_none(),
                    cut([centre[0], centre[1]]),
                    "cell ({i}, {j}) follows the rule"
                );
            }
        }
        assert_eq!(geometry.nodes().len(), nx * ny - expected);
    }

    /// The tie is a change to the mesh: the cells inside the rectangle
    /// share one node at the mean of their centres, the bars that ran
    /// between them are gone, and the bars crossing the boundary now end on
    /// that node.
    #[test]
    fn an_equipotential_ties_its_cells_into_one_node() {
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        // Cell centres sit on a 2 mm grid: x = 1, 3, 5, 7, 9 and
        // y = 1, 3, 5 mm. This rectangle catches the six centres with
        // x ∈ {3, 5, 7} and y ∈ {1, 3}.
        plane
            .equipotentials
            .push(Equipotential::new([2e-3, 0.0], [8e-3, 4e-3]));
        let mesh = plane.mesh().unwrap();
        let centres = plane.build_into(&mut geometry).unwrap();

        let tie = centres[1][0].unwrap();
        for (i, j) in [(1, 0), (1, 1), (2, 0), (2, 1), (3, 0), (3, 1)] {
            assert_eq!(centres[i][j], Some(tie), "cell ({i}, {j}) joins the tie");
        }
        for (i, j) in [(0, 0), (4, 0), (0, 1), (4, 1), (0, 2), (2, 2), (4, 2)] {
            assert!(
                centres[i][j].is_some() && centres[i][j] != Some(tie),
                "cell ({i}, {j}) keeps its own node"
            );
        }
        // Six cells became one node; the tie sits at the mean of the six
        // centres, at the mesh's own mid-thickness depth.
        assert_eq!(geometry.nodes().len(), 15 - 6 + 1);
        let node = geometry.nodes()[tie.0];
        assert!((node.x - 5e-3).abs() < 1e-12, "{}", node.x);
        assert!((node.y - 2e-3).abs() < 1e-12, "{}", node.y);
        assert!((node.z + 17.5e-6).abs() < 1e-12);
        // The seven bars inside the patch (four across x, three across y)
        // are not built; every other bar survives, including the ones that
        // cross the patch's boundary.
        assert_eq!(geometry.segment_count(), 22 - 7);
        assert_eq!(geometry.segment_count(), plane.live_bars(&mesh));
        // No bar joins a node to itself.
        for index in 0..geometry.segment_count() {
            let def = geometry.segment_defs()[index];
            assert_ne!(def.a, def.b, "segment {index} is a self-loop");
        }
        // The patch's node is what a deck naming this contact area gets…
        assert_eq!(plane.equipotential_node(&centres, 0).unwrap(), Some(tie));
        // …and an endpoint landing anywhere inside the patch snaps to it.
        assert_eq!(plane.attach(&centres, [6.9e-3, 0.1e-3, 0.0]).unwrap(), tie);
    }

    /// The rectangle's boundary is closed: a cell centre exactly on an edge
    /// (or corner) is tied, unlike [`Hole::Rect`]'s strict interior.
    #[test]
    fn the_equipotential_boundary_includes_its_edges() {
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        // Corners exactly on the centres (5, 1), (5, 3), (7, 1), (7, 3) mm.
        plane
            .equipotentials
            .push(Equipotential::new([5e-3, 1e-3], [7e-3, 3e-3]));
        let centres = plane.build_into(&mut geometry).unwrap();
        let tie = centres[2][0].unwrap();
        for (i, j) in [(2, 0), (3, 0), (2, 1), (3, 1)] {
            assert_eq!(centres[i][j], Some(tie), "cell ({i}, {j}) is on the patch");
        }
        assert_eq!(geometry.nodes().len(), 15 - 4 + 1);
    }

    /// A rectangle that catches no live cell centre ties nothing — whether
    /// it falls between centres or a hole has taken the only cell it
    /// covers. The mesh is then exactly the untied one.
    #[test]
    fn an_equipotential_catching_no_live_cell_ties_nothing() {
        // Between the centres at x = 1 and x = 3 mm.
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        plane
            .equipotentials
            .push(Equipotential::new([1.2e-3, 0.2e-3], [2.8e-3, 1.8e-3]));
        let centres = plane.build_into(&mut geometry).unwrap();
        assert_eq!(geometry.nodes().len(), 15);
        assert_eq!(geometry.segment_count(), 22);
        assert_eq!(plane.equipotential_node(&centres, 0).unwrap(), None);

        // Over cell (2, 1) alone, with a hole that has already taken it.
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        plane.holes.push(Hole::Point { at: [5e-3, 3.5e-3] });
        plane
            .equipotentials
            .push(Equipotential::centred([5e-3, 3e-3], [1e-3, 1e-3]));
        let centres = plane.build_into(&mut geometry).unwrap();
        assert!(centres[2][1].is_none());
        assert_eq!(geometry.nodes().len(), 14);
        assert_eq!(plane.equipotential_node(&centres, 0).unwrap(), None);
    }

    /// Two rectangles that tie a cell in common are one equipotential —
    /// two overlapping perfect conductors are one conductor — while
    /// disjoint ones stay separate nodes.
    #[test]
    fn equipotentials_sharing_a_cell_merge() {
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        // The first catches the bottom-row centres x = 1, 3, 5 mm, the
        // second x = 5, 7, 9 — the centre at x = 5 mm is in both, so all
        // five merge into one equipotential.
        plane
            .equipotentials
            .push(Equipotential::new([0.5e-3, 0.0], [5.5e-3, 2e-3]));
        plane
            .equipotentials
            .push(Equipotential::new([4.5e-3, 0.0], [9.5e-3, 2e-3]));
        let mesh = plane.mesh().unwrap();
        let centres = plane.build_into(&mut geometry).unwrap();
        let tie = centres[0][0].unwrap();
        for (i, column) in centres.iter().enumerate() {
            assert_eq!(column[0], Some(tie), "cell ({i}, 0) joins the tie");
        }
        assert_eq!(geometry.nodes().len(), 15 - 5 + 1);
        assert_eq!(geometry.segment_count(), plane.live_bars(&mesh));
        // Either region names the merged node.
        assert_eq!(plane.equipotential_node(&centres, 0).unwrap(), Some(tie));
        assert_eq!(plane.equipotential_node(&centres, 1).unwrap(), Some(tie));
        // The merged tie sits at the mean of the whole row it ties.
        let node = geometry.nodes()[tie.0];
        assert!((node.x - 5e-3).abs() < 1e-12, "{}", node.x);

        // Regions that share no cell stay two nodes.
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        plane
            .equipotentials
            .push(Equipotential::centred([1e-3, 1e-3], [1e-3, 1e-3]));
        plane
            .equipotentials
            .push(Equipotential::centred([9e-3, 5e-3], [1e-3, 1e-3]));
        let centres = plane.build_into(&mut geometry).unwrap();
        let first = plane.equipotential_node(&centres, 0).unwrap().unwrap();
        let second = plane.equipotential_node(&centres, 1).unwrap().unwrap();
        assert_ne!(first, second);
        // Each covers one cell, so the mesh is untouched but for the ties.
        assert_eq!(geometry.nodes().len(), 15);
    }

    /// A tie composes with a contact region on the same rectangle — the
    /// pair a deck's `contact connection` writes: the region's fine cells
    /// are all tied, and the graded mesh is otherwise the region's own.
    #[test]
    fn an_equipotential_composes_with_a_contact_region() {
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        plane
            .contacts
            .push(ContactRegion::centred([5e-3, 3e-3], 1e-3, 4, 2.0));
        plane
            .equipotentials
            .push(Equipotential::centred([5e-3, 3e-3], [1e-3, 1e-3]));
        let mesh = plane.mesh().unwrap();
        let centres = plane.build_into(&mut geometry).unwrap();
        let tie = plane.equipotential_node(&centres, 0).unwrap().unwrap();

        let mut tied = 0;
        for (i, column) in centres.iter().enumerate() {
            for (j, slot) in column.iter().enumerate() {
                let centre = mesh.centre(i, j);
                let inside = (4.5e-3..=5.5e-3).contains(&centre[0])
                    && (2.5e-3..=3.5e-3).contains(&centre[1]);
                assert_eq!(*slot == Some(tie), inside, "cell ({i}, {j}) at {centre:?}");
                tied += usize::from(inside);
            }
        }
        assert_eq!(tied, 16, "the region's 4 × 4 fine cells are all tied");
        assert_eq!(geometry.nodes().len(), mesh.nx() * mesh.ny() - tied + 1);
        assert_eq!(geometry.segment_count(), plane.live_bars(&mesh));
    }

    #[test]
    fn attach_snaps_to_the_nearest_live_centre() {
        let mut geometry = Geometry::new();
        let mut plane = test_plane();
        // A hole over the (0, 0) cell's centre (1 mm, 1 mm): that cell
        // dies; a point over the holed corner then snaps to the nearest
        // live centre, cell (0, 1) at (1 mm, 3 mm) — ties resolved by
        // grid order.
        plane.holes.push(Hole::Rect {
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

    /// A per-axis ratio decays the two axes at different rates — the case
    /// a single scalar ratio cannot express. On the 10 × 6 mm test plane
    /// (2 mm background cell) a 2 × 2 mm region at 0.5 mm fine cells is
    /// ungraded across x (ratio 1, so 0.5 mm everywhere: 20 cells) and
    /// decays in one step across y (ratio 4, so 0.5 mm × 4 = the 2 mm
    /// background cell immediately: 1 + 4 + 1 cells).
    #[test]
    fn a_per_axis_ratio_decays_each_axis_at_its_own_rate() {
        let region =
            |ratio| ContactRegion::graded_per_axis([4e-3, 2e-3], [6e-3, 4e-3], [4, 4], ratio);
        let mesh = GroundPlane {
            contacts: vec![region([1.0, 4.0])],
            ..test_plane()
        }
        .mesh()
        .unwrap();
        assert_eq!((mesh.nx(), mesh.ny()), (20, 6));

        // Swapping the ratios swaps the two axes' layouts: across x the
        // two 4 mm gaps each take two background cells (2 + 4 + 2), and y
        // is now the uniformly fine axis (6 mm / 0.5 mm).
        let mesh = GroundPlane {
            contacts: vec![region([4.0, 1.0])],
            ..test_plane()
        }
        .mesh()
        .unwrap();
        assert_eq!((mesh.nx(), mesh.ny()), (8, 12));

        // `new` is the isotropic special case of `graded_per_axis`.
        assert_eq!(
            ContactRegion::new([4e-3, 2e-3], [6e-3, 4e-3], [4, 4], 1.5),
            region([1.5, 1.5])
        );
    }

    #[test]
    fn contacts_compose_with_holes() {
        let plane = GroundPlane {
            contacts: vec![ContactRegion::centred([5e-3, 3e-3], 1e-3, 2, 2.0)],
            holes: vec![Hole::Rect {
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

    /// A band widened to absorb a sliver at the footprint edge, or merged
    /// with a neighbour, is cut into the fewest cells *no coarser* than its
    /// fine cell — never into the nearest count, which could leave a cell
    /// up to twice as wide as the region asked for.
    #[test]
    fn widened_and_merged_bands_never_coarsen_the_fine_cell() {
        // A one-cell, 1 mm region 0.3 mm from the plane's left edge: the
        // 0.3 mm sliver is absorbed, and the 1.3 mm band takes two cells
        // (rounding would have made it one 1.3 mm cell).
        let plane = GroundPlane {
            contacts: vec![ContactRegion::new(
                [0.3e-3, 2.5e-3],
                [1.3e-3, 3.5e-3],
                [1, 1],
                2.0,
            )],
            ..test_plane()
        };
        let mesh = plane.mesh().unwrap();
        assert!((mesh.dx(0) - 0.65e-3).abs() < 1e-12, "{}", mesh.dx(0));
        assert!((mesh.dx(1) - 0.65e-3).abs() < 1e-12, "{}", mesh.dx(1));

        // Two regions 0.1 mm apart merge into one 1.6 mm band at the finer
        // 0.25 mm cell: 6.4 cells' worth, so 7 cells, none above 0.25 mm.
        let plane = GroundPlane {
            contacts: vec![
                ContactRegion::new([4e-3, 2e-3], [5e-3, 4e-3], [4, 4], 2.0),
                ContactRegion::new([5.1e-3, 2e-3], [5.6e-3, 4e-3], [1, 4], 2.0),
            ],
            ..test_plane()
        };
        let mesh = plane.mesh().unwrap();
        let band: Vec<f64> = (0..mesh.nx())
            .filter(|&i| {
                mesh.x_edges()[i] >= 4e-3 - 1e-12 && mesh.x_edges()[i + 1] <= 5.6e-3 + 1e-12
            })
            .map(|i| mesh.dx(i))
            .collect();
        assert_eq!(band.len(), 7, "{band:?}");
        assert!(band.iter().all(|&d| d <= 0.25e-3 + 1e-12), "{band:?}");
    }

    /// A band spanning the whole axis at the plane's own cell count is the
    /// background mesh, not a refinement, so it is dropped before the merge
    /// pass instead of widening every finer band on the axis across the
    /// whole of it. On this 10 × 6 mm / 2 mm-cell plane, the whole-plane x
    /// band a one-axis-met `contact point` emits beside a 0.1 mm region near
    /// (1, 1) mm once cost 100 x cells — the finest cell across the entire
    /// axis — against the 11 the 0.1 mm region costs on its own. Issue #124.
    #[test]
    fn a_whole_axis_background_band_never_widens_a_finer_one() {
        // `contact point (5.3, 3, 0, 4, 0.5)`: x is already met by the 2 mm
        // background, so it spans the plane at nx = 5; y is refined.
        let met_on_x = ContactRegion::new([0.0, 2.75e-3], [10e-3, 3.25e-3], [5, 1], 2.0);
        // A genuine 0.1 mm refinement, nowhere near it on either axis.
        let fine = ContactRegion::new([0.95e-3, 0.95e-3], [1.05e-3, 1.05e-3], [1, 1], 2.0);
        let mesh = |contacts: Vec<ContactRegion>| {
            GroundPlane {
                contacts,
                ..test_plane()
            }
            .mesh()
            .unwrap()
        };

        let alone = mesh(vec![fine]);
        let both = mesh(vec![met_on_x, fine]);
        assert_eq!(alone.nx(), 11, "the 0.1 mm region's own x cost");
        assert_eq!(
            both.nx(),
            alone.nx(),
            "the met x axis costs what the 0.1 mm region alone costs"
        );
        // 10 mm / 0.1 mm = the 100 cells the merge once spent on the axis.
        assert!(
            both.nx() < 100,
            "the axis is not refined to 0.1 mm throughout ({})",
            both.nx()
        );
        for (a, b) in both.x_edges().iter().zip(alone.x_edges()) {
            assert!((a - b).abs() < 1e-15, "x edge {a} vs {b}");
        }
        // The y refinement the clause did ask for is untouched.
        assert!(both.ny() > alone.ny(), "y is still refined by the point");

        // On its own, the whole-plane band is the plain plane's x axis, edge
        // for edge — what dropping it has to preserve (issue #116).
        let clamped = mesh(vec![met_on_x]);
        let plain = mesh(Vec::new());
        assert_eq!(clamped.nx(), plain.nx());
        for (a, b) in clamped.x_edges().iter().zip(plain.x_edges()) {
            assert!((a - b).abs() < 1e-15, "x edge {a} vs {b}");
        }
    }

    /// The disposition for an explicit [`ContactRegion`] cut at the
    /// background cell: only the *whole-axis* band is a no-op and dropped.
    /// One over part of an axis is kept, so it still pins its own edges —
    /// even beside a finer band whose grading would otherwise move them.
    /// Issue #124.
    #[test]
    fn a_part_axis_background_band_still_pins_its_edges() {
        // 2 … 4 mm at exactly the 2 mm background cell, with a 0.1 mm region
        // at the far end of the axis to grade the gap between them.
        let plane = GroundPlane {
            contacts: vec![
                ContactRegion::new([2e-3, 2e-3], [4e-3, 4e-3], [1, 1], 2.0),
                ContactRegion::new([8.95e-3, 2.95e-3], [9.05e-3, 3.05e-3], [1, 1], 2.0),
            ],
            ..test_plane()
        };
        let mesh = plane.mesh().unwrap();
        for pin in [2e-3, 4e-3] {
            assert!(
                mesh.x_edges().iter().any(|e| (e - pin).abs() < 1e-12),
                "the band's own edge at {pin} m is pinned: {:?}",
                mesh.x_edges()
            );
        }
        // Dropping it instead would grade that gap straight past those
        // edges: without the band the same plane has neither.
        let ungraded = GroundPlane {
            contacts: vec![ContactRegion::new(
                [8.95e-3, 2.95e-3],
                [9.05e-3, 3.05e-3],
                [1, 1],
                2.0,
            )],
            ..test_plane()
        }
        .mesh()
        .unwrap();
        assert!(
            !ungraded
                .x_edges()
                .iter()
                .any(|e| (e - 2e-3).abs() < 1e-12 || (e - 4e-3).abs() < 1e-12),
            "the pins come from the band, not from the grading: {:?}",
            ungraded.x_edges()
        );
    }

    /// A contact meeting the footprint only at an edge — exactly, or by a
    /// rounding-sized overlap — is rejected deterministically on every
    /// edge and axis, at any unit scale and for a translated plane, while
    /// a genuinely small overlap still refines and builds (issue #134).
    #[test]
    fn touching_and_sliver_contacts_are_handled_deterministically() {
        for (scale, offset) in [
            (1.0, [0.0, 0.0]),
            (1e3, [0.0, 0.0]),
            (1e-3, [0.0, 0.0]),
            (1.0, [5.0, -3.0]),
            (1.0, [1e9, -1e9]),
        ] {
            let lo = [offset[0], offset[1]];
            let hi = [offset[0] + 10e-3 * scale, offset[1] + 6e-3 * scale];
            let plane = |contact: ContactRegion| GroundPlane {
                lo,
                hi,
                z_top: 0.0,
                thickness: 35e-6 * scale,
                nx: 5,
                ny: 3,
                contacts: vec![contact],
                ..test_plane()
            };
            let span = [hi[0] - lo[0], hi[1] - lo[1]];
            let width = [2e-3 * scale, 2e-3 * scale];
            // A rectangle on the far side of `edge` of `axis`, reaching
            // `overlap` back across it; `upper` picks the hi edge.
            let region = |axis: usize, upper: bool, overlap: f64| {
                let mut a = [lo[0] + 3e-3 * scale, lo[1] + 2e-3 * scale];
                let mut b = [a[0] + width[0], a[1] + width[1]];
                if upper {
                    a[axis] = hi[axis] - overlap;
                } else {
                    a[axis] = lo[axis] + overlap - width[axis];
                }
                b[axis] = a[axis] + width[axis];
                ContactRegion::new(a, b, [2, 2], 2.0)
            };
            for axis in 0..2 {
                for upper in [false, true] {
                    // Exact touch, rounding noise, and one representable
                    // neighbour either way of the edge.
                    let edge = if upper { hi[axis] } else { lo[axis] };
                    let noise = 4.0 * f64::EPSILON * edge.abs().max(span[axis]);
                    for overlap in [0.0, noise / 2.0, noise] {
                        let error = plane(region(axis, upper, overlap)).mesh().unwrap_err();
                        assert!(
                            matches!(error, PlaneError::ContactOutsideFootprint { .. }),
                            "scale {scale} offset {offset:?} axis {axis} upper {upper} overlap {overlap}: {error}"
                        );
                    }
                    // A genuinely small refinement survives: a thousandth
                    // of the span, or — far from the origin, where that is
                    // only a few dozen ULPs — a thousand ULPs of the edge.
                    let overlap = (1e-3 * span[axis]).max(1e3 * f64::EPSILON * edge.abs());
                    let candidate = plane(region(axis, upper, overlap));
                    let mesh = candidate.mesh().unwrap();
                    for edges in [mesh.x_edges(), mesh.y_edges()] {
                        assert!(edges.windows(2).all(|w| w[1] > w[0]), "{edges:?}");
                    }
                    let mut geometry = Geometry::new();
                    candidate.build_into(&mut geometry).unwrap();
                }
            }
        }
    }

    /// A small plane translated far from the origin keeps its genuine
    /// partial overlaps: the touch slack separates shape-relative tolerance
    /// from coordinate rounding, so it is a few ULPs of the coordinates,
    /// not a fraction of their magnitude that would exceed the whole span
    /// (issue #134 review).
    #[test]
    fn translated_small_plane_keeps_partial_overlaps() {
        let lo = [1e9, 1e9];
        let hi = [1e9 + 10e-3, 1e9 + 6e-3];
        let slack = contact_slack(lo[0], hi[0]);
        let span = hi[0] - lo[0];
        assert!(slack < 1e-3 * span, "slack {slack} vs span {span}");
        // Still wider than the coordinates' own rounding noise.
        let ulp = f64::EPSILON * hi[0];
        assert!(slack > 4.0 * ulp, "slack {slack} vs ulp {ulp}");
        let plane = |contact: ContactRegion| GroundPlane {
            lo,
            hi,
            z_top: 0.0,
            thickness: 35e-6,
            nx: 5,
            ny: 3,
            contacts: vec![contact],
            ..test_plane()
        };
        // A meaningful partial overlap (a quarter of the 2 mm contact,
        // across the hi-x edge) meshes and builds.
        let partial = ContactRegion::new(
            [hi[0] - 0.5e-3, lo[1] + 2e-3],
            [hi[0] + 1.5e-3, lo[1] + 4e-3],
            [2, 2],
            2.0,
        );
        let candidate = plane(partial);
        let mesh = candidate.mesh().unwrap();
        for edges in [mesh.x_edges(), mesh.y_edges()] {
            assert!(edges.windows(2).all(|w| w[1] > w[0]), "{edges:?}");
        }
        let mut geometry = Geometry::new();
        candidate.build_into(&mut geometry).unwrap();
        // An ULP-sized sliver across the same edge is still a touch.
        let below = hi[0] - 2.0 * ulp;
        assert!(below < hi[0]);
        let sliver = ContactRegion::new(
            [below, lo[1] + 2e-3],
            [below + 2e-3, lo[1] + 4e-3],
            [2, 2],
            2.0,
        );
        let error = plane(sliver).mesh().unwrap_err();
        assert!(
            matches!(error, PlaneError::ContactOutsideFootprint { .. }),
            "{error}"
        );
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
