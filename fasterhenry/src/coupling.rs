//! Coupling truncation: naming groups of segments and declaring which pairs
//! of groups are magnetically coupled, so that the mutual inductance of the
//! rest is never computed.
//!
//! This is the library side of FastHenry's `.couples` knob. Segments carry a
//! group name; a [`Coupling`] says which groups couple to which. Assembly
//! ([`MeshSystem::assemble_with_coupling`](crate::solve::MeshSystem::assemble_with_coupling))
//! then skips the kernel entirely for every filament pair whose owning
//! segments sit in two groups that were not declared coupled, and stores zero
//! in those entries of `L`.
//!
//! The default — [`Coupling::all_pairs`], which is also what plain
//! [`MeshSystem::assemble`](crate::solve::MeshSystem::assemble) uses — couples
//! everything to everything, exactly as before.
//!
//! ```
//! use fasterhenry::coupling::Coupling;
//!
//! // Two boards in one deck; their mutual inductance is not wanted.
//! let coupling = Coupling::truncated(vec![
//!     "left".to_owned(),
//!     "left".to_owned(),
//!     "right".to_owned(),
//! ]);
//! assert!(!coupling.is_all_pairs());
//!
//! // …unless it is, in which case declare it and pay for it.
//! let coupling = coupling.coupled("left", "right");
//! assert!(coupling.couples("left", "right"));
//! ```
//!
//! # What it buys
//!
//! Assembly costs one kernel evaluation per unordered filament pair, so a
//! problem of `n` filaments costs `n(n+1)/2` of them. Splitting it into two
//! uncoupled groups of `n/2` removes the `n²/4` cross pairs — a little under
//! half the work — and `g` equal, mutually uncoupled groups leave `1/g` of it.
//! [`PairMask::kept_pairs`] reports the exact count before anything is
//! evaluated.
//!
//! The memory of the assembled `L` is unchanged: this is a truncation of the
//! *work*, not yet a sparse matrix format.
//!
//! # When truncation is safe
//!
//! Dropping `M` between two groups is an error of `M` — there is no
//! cancellation to rescue it — so the question is only how large `M` is
//! compared with the self terms that remain.
//!
//! For two current loops whose separation `d` is large compared with their
//! own extent `a`, the leading term of the Neumann integral is the
//! dipole–dipole one,
//!
//! ```text
//! M ~ μ0·A₁A₂ / (4π·d³) ,     A ~ a²  ⇒  M/L ~ (a/d)³ ,
//! ```
//!
//! so the relative error falls as the cube of the separation: ten extents
//! apart is a part in `10³` of the loop inductance, and the crate's
//! `tests/coupling.rs` two-island fixture measures exactly that. **Closed
//! current loops more than about ten times their own extent apart are safe to
//! truncate.**
//!
//! Two caveats, in decreasing order of how often they bite:
//!
//! * **Open conductors are not dipoles.** Two straight traces that do not
//!   close their own return path couple as `M ~ μ0·l²/(4π·d)` — falling as
//!   `1/d`, not `1/d³`. Ten lengths apart is then still about a percent of the
//!   self inductance, and a hundred is needed for a part in a thousand.
//!   Truncate groups that each carry their own return current.
//! * **Coupling declarations should be transitive.** The truncated `L` stays
//!   symmetric, and it stays positive semidefinite whenever the "is coupled
//!   to" relation is an equivalence (the groups then form independent blocks).
//!   Declaring `a`–`b` and `b`–`c` coupled but not `a`–`c` can in principle
//!   produce an indefinite `L` and an unphysical, even singular, solve. Prefer
//!   declaring the whole clique.
//!
//! [`truncation_warnings`](Coupling::truncation_warnings) reports every
//! truncated pair whose bounding boxes lie closer than one extent apart —
//! the regime in which the approximation is certainly not justified. A silent
//! run is not a proof of accuracy: it only means nothing was obviously wrong.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::geometry::Geometry;
use crate::inductance::PairMask;

/// The group a segment belongs to when it names none: the empty name.
pub const DEFAULT_GROUP: &str = "";

/// Why a [`Coupling`] does not fit its geometry.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CouplingError {
    /// A declared coupling names a group no segment belongs to — a typo
    /// rather than a no-op.
    #[error("coupling declares group '{name}', which no segment belongs to")]
    UnknownGroup {
        /// The name as declared.
        name: String,
    },
    /// More group tags than the geometry has segments.
    #[error("coupling names {got} segment groups, but the geometry has {expected} segments")]
    SegmentCountMismatch {
        /// Segments in the geometry.
        expected: usize,
        /// Group tags in the coupling.
        got: usize,
    },
    /// Two groups are reachable through other declared couplings, but are not
    /// declared coupled directly — the "is coupled to" relation is not
    /// transitive, and `L` may fail to be positive semidefinite. See the
    /// module docs for why the whole clique should be declared.
    #[error(
        "coupling is not transitive: groups '{a}' and '{b}' are only reachable through a third \
         group, but are not declared coupled directly — declare the whole clique",
        a = groups[0],
        b = groups[1]
    )]
    NonTransitiveCoupling {
        /// The two groups connected only indirectly.
        groups: [String; 2],
    },
}

/// A truncated pair of groups that are too close for the approximation to be
/// obviously justified.
#[derive(Clone, Debug, PartialEq)]
pub struct TruncationWarning {
    /// The two groups whose mutual inductance was dropped.
    pub groups: [String; 2],
    /// Gap between their bounding boxes, in metres; zero if they overlap.
    pub separation: f64,
    /// The larger of the two bounding-box diagonals, in metres — the length
    /// scale `separation` is judged against.
    pub extent: f64,
}

impl fmt::Display for TruncationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = |group: &str| {
            if group.is_empty() {
                "(default)".to_owned()
            } else {
                format!("'{group}'")
            }
        };
        let ratio = if self.extent > 0.0 {
            self.separation / self.extent
        } else {
            f64::INFINITY
        };
        write!(
            f,
            "mutual inductance between groups {} and {} is truncated, but they are only {:.3e} m apart — {:.2}x their {:.3e} m extent; truncation is justified from about 10x",
            name(&self.groups[0]),
            name(&self.groups[1]),
            self.separation,
            ratio,
            self.extent
        )
    }
}

/// Segment group names and the group pairs whose mutual inductance is kept.
///
/// See the [module documentation](self) for the semantics and for when
/// truncation is safe.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coupling {
    /// Group name of each segment, in geometry segment order. Segments past
    /// the end of this list, and entries equal to [`DEFAULT_GROUP`], belong to
    /// the default group.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    segment_groups: Vec<String>,
    /// Cross-group pairs to keep. `None` — the default — keeps every pair and
    /// ignores the group names entirely.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    couples: Option<Vec<[String; 2]>>,
}

impl Coupling {
    /// Couple everything to everything: the default, and what assembly did
    /// before this knob existed.
    pub fn all_pairs() -> Self {
        Self::default()
    }

    /// Tag each segment with a group (in geometry segment order) and keep
    /// only the pairs declared with [`coupled`](Self::coupled) — every group
    /// with itself, and nothing else until declared.
    pub fn truncated(segment_groups: Vec<String>) -> Self {
        Self {
            segment_groups,
            couples: Some(Vec::new()),
        }
    }

    /// Declares that groups `a` and `b` are coupled, so their mutual
    /// inductance is computed in full.
    ///
    /// Switches an [`all_pairs`](Self::all_pairs) coupling into a truncating
    /// one, since declaring a pair is only meaningful when the rest are
    /// dropped.
    #[must_use]
    pub fn coupled(mut self, a: impl Into<String>, b: impl Into<String>) -> Self {
        let pair = [a.into(), b.into()];
        self.couples.get_or_insert_with(Vec::new).push(pair);
        self
    }

    /// Whether every pair is kept, so assembly is unaffected.
    pub fn is_all_pairs(&self) -> bool {
        self.couples.is_none()
    }

    /// Whether this is the untouched default: no group tags, no truncation.
    pub fn is_trivial(&self) -> bool {
        self.segment_groups.is_empty() && self.couples.is_none()
    }

    /// The group names, in geometry segment order.
    pub fn segment_groups(&self) -> &[String] {
        &self.segment_groups
    }

    /// The declared cross-group couplings, or `None` when nothing is
    /// truncated.
    pub fn declared_couples(&self) -> Option<&[[String; 2]]> {
        self.couples.as_deref()
    }

    /// The group of segment `index`; [`DEFAULT_GROUP`] when it carries no tag.
    pub fn group_of(&self, index: usize) -> &str {
        self.segment_groups
            .get(index)
            .map_or(DEFAULT_GROUP, String::as_str)
    }

    /// Whether the mutual inductance of groups `a` and `b` is kept. A group is
    /// always coupled to itself, and everything is coupled under
    /// [`all_pairs`](Self::all_pairs).
    pub fn couples(&self, a: &str, b: &str) -> bool {
        match &self.couples {
            None => true,
            Some(_) if a == b => true,
            Some(pairs) => pairs
                .iter()
                .any(|[x, y]| (x == a && y == b) || (x == b && y == a)),
        }
    }

    /// Checks the coupling against a geometry of `segment_count` segments:
    /// no more tags than segments, and every declared name in use.
    ///
    /// # Errors
    ///
    /// [`CouplingError::SegmentCountMismatch`] or
    /// [`CouplingError::UnknownGroup`].
    pub fn validate(&self, segment_count: usize) -> Result<(), CouplingError> {
        self.resolve(segment_count).map(drop)
    }

    /// The filament-level mask for a geometry whose segments were cut into
    /// `filaments_per_segment[i]` filaments each.
    ///
    /// # Errors
    ///
    /// As [`validate`](Self::validate).
    pub fn pair_mask(&self, filaments_per_segment: &[usize]) -> Result<PairMask, CouplingError> {
        let resolved = self.resolve(filaments_per_segment.len())?;
        let mut group = Vec::with_capacity(filaments_per_segment.iter().sum());
        for (segment, &count) in filaments_per_segment.iter().enumerate() {
            group.extend(std::iter::repeat_n(resolved.of_segment[segment], count));
        }
        Ok(PairMask::from_groups(
            group,
            resolved.names.len(),
            |a, b| resolved.couples(a, b),
        ))
    }

    /// Every truncated group pair whose bounding boxes lie closer than one
    /// extent apart, in group order.
    ///
    /// A group's extent is the diagonal of the axis-aligned bounding box of
    /// its segments' end points; the separation is the gap between the two
    /// boxes, zero when they overlap. Empty under
    /// [`all_pairs`](Self::all_pairs).
    ///
    /// # Errors
    ///
    /// As [`validate`](Self::validate).
    pub fn truncation_warnings(
        &self,
        geometry: &Geometry,
    ) -> Result<Vec<TruncationWarning>, CouplingError> {
        let resolved = self.resolve(geometry.segment_count())?;
        if self.is_all_pairs() {
            return Ok(Vec::new());
        }
        let mut boxes = vec![BoundingBox::EMPTY; resolved.names.len()];
        for (index, segment) in geometry.segments().enumerate() {
            let box_ = &mut boxes[resolved.of_segment[index]];
            box_.add([segment.a.x, segment.a.y, segment.a.z]);
            box_.add([segment.b.x, segment.b.y, segment.b.z]);
        }

        let mut warnings = Vec::new();
        for a in 0..resolved.names.len() {
            for b in (a + 1)..resolved.names.len() {
                if resolved.couples(a, b) {
                    continue;
                }
                let (first, second) = (&boxes[a], &boxes[b]);
                let extent = first.diagonal().max(second.diagonal());
                let separation = first.gap(second);
                if separation < extent {
                    warnings.push(TruncationWarning {
                        groups: [resolved.names[a].clone(), resolved.names[b].clone()],
                        separation,
                        extent,
                    });
                }
            }
        }
        Ok(warnings)
    }

    /// Group indices per segment, the names in index order, and the coupled
    /// table.
    fn resolve(&self, segment_count: usize) -> Result<Resolved, CouplingError> {
        if self.segment_groups.len() > segment_count {
            return Err(CouplingError::SegmentCountMismatch {
                expected: segment_count,
                got: self.segment_groups.len(),
            });
        }
        let mut indices: HashMap<&str, usize> = HashMap::new();
        let mut names: Vec<String> = Vec::new();
        let mut of_segment = Vec::with_capacity(segment_count);
        for index in 0..segment_count {
            let name = self.group_of(index);
            let next = names.len();
            let group = *indices.entry(name).or_insert_with(|| {
                names.push(name.to_owned());
                next
            });
            of_segment.push(group);
        }

        let groups = names.len();
        let mut couples = vec![false; groups * groups];
        for (index, slot) in couples.iter_mut().enumerate() {
            *slot = index / groups == index % groups;
        }
        if let Some(pairs) = &self.couples {
            for [a, b] in pairs {
                let lookup = |name: &String| {
                    indices
                        .get(name.as_str())
                        .copied()
                        .ok_or_else(|| CouplingError::UnknownGroup { name: name.clone() })
                };
                let (a, b) = (lookup(a)?, lookup(b)?);
                couples[a * groups + b] = true;
                couples[b * groups + a] = true;
            }
        } else {
            couples.fill(true);
        }
        check_transitive(&couples, &names)?;
        Ok(Resolved {
            of_segment,
            names,
            couples,
        })
    }
}

/// Rejects a declared "is coupled to" relation that is not transitive: some
/// pair of groups reachable only through a third group, but not declared
/// coupled directly.
///
/// For a symmetric relation, "transitive closure" and "connected component"
/// coincide, so this computes connected components of the declared-pairs
/// graph via union-find, then checks that each component with more than one
/// group is a complete subgraph — every pair within it directly declared.
fn check_transitive(couples: &[bool], names: &[String]) -> Result<(), CouplingError> {
    let groups = names.len();
    if groups < 3 {
        // Fewer than three groups: no third group to be reachable through,
        // so the relation is trivially transitive.
        return Ok(());
    }

    let mut parent: Vec<usize> = (0..groups).collect();
    fn find(parent: &mut [usize], x: usize) -> usize {
        if parent[x] != x {
            parent[x] = find(parent, parent[x]);
        }
        parent[x]
    }

    for a in 0..groups {
        for b in (a + 1)..groups {
            if couples[a * groups + b] {
                let (root_a, root_b) = (find(&mut parent, a), find(&mut parent, b));
                if root_a != root_b {
                    parent[root_a] = root_b;
                }
            }
        }
    }

    for a in 0..groups {
        for b in (a + 1)..groups {
            if couples[a * groups + b] {
                continue;
            }
            if find(&mut parent, a) == find(&mut parent, b) {
                return Err(CouplingError::NonTransitiveCoupling {
                    groups: [names[a].clone(), names[b].clone()],
                });
            }
        }
    }
    Ok(())
}

/// A [`Coupling`] resolved against a segment count.
struct Resolved {
    of_segment: Vec<usize>,
    names: Vec<String>,
    couples: Vec<bool>,
}

impl Resolved {
    fn couples(&self, a: usize, b: usize) -> bool {
        self.couples[a * self.names.len() + b]
    }
}

/// An axis-aligned bounding box, grown point by point.
#[derive(Clone, Copy, Debug)]
struct BoundingBox {
    lo: [f64; 3],
    hi: [f64; 3],
}

impl BoundingBox {
    const EMPTY: Self = Self {
        lo: [f64::INFINITY; 3],
        hi: [f64::NEG_INFINITY; 3],
    };

    fn add(&mut self, point: [f64; 3]) {
        for (axis, coordinate) in point.into_iter().enumerate() {
            self.lo[axis] = self.lo[axis].min(coordinate);
            self.hi[axis] = self.hi[axis].max(coordinate);
        }
    }

    fn is_empty(&self) -> bool {
        (0..3).any(|axis| self.lo[axis] > self.hi[axis])
    }

    /// Length of the box diagonal; zero when no point was ever added.
    fn diagonal(&self) -> f64 {
        if self.is_empty() {
            return 0.0;
        }
        (0..3)
            .map(|axis| (self.hi[axis] - self.lo[axis]).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// Distance between two boxes, zero when they overlap or touch.
    fn gap(&self, other: &Self) -> f64 {
        if self.is_empty() || other.is_empty() {
            return f64::INFINITY;
        }
        (0..3)
            .map(|axis| {
                let apart = (self.lo[axis] - other.hi[axis]).max(other.lo[axis] - self.hi[axis]);
                apart.max(0.0).powi(2)
            })
            .sum::<f64>()
            .sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Node, NodeId, SegmentDef};

    const COPPER: f64 = 5.8e7;

    fn groups(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    /// Two parallel traces of length 1 along x, `separation` apart in y,
    /// tagged `a` and `b`.
    fn two_traces(separation: f64) -> Geometry {
        let mut geometry = Geometry::new();
        for y in [0.0, separation] {
            let a = NodeId(geometry.add_node(Node::new(0.0, y, 0.0)).unwrap().0);
            let b = NodeId(geometry.add_node(Node::new(1.0, y, 0.0)).unwrap().0);
            geometry
                .add_segment(SegmentDef::new(a, b, 0.1, 0.1, COPPER))
                .unwrap();
        }
        geometry
    }

    #[test]
    fn the_default_couples_everything() {
        let coupling = Coupling::all_pairs();
        assert!(coupling.is_all_pairs());
        assert!(coupling.is_trivial());
        assert!(coupling.couples("a", "b"));
        assert_eq!(coupling.group_of(7), DEFAULT_GROUP);

        let mask = coupling.pair_mask(&[2, 3]).unwrap();
        assert!(mask.keeps_everything());
        assert_eq!(mask.kept_pairs(), mask.total_pairs());
        assert_eq!(mask.len(), 5);
    }

    #[test]
    fn truncation_drops_exactly_the_cross_group_pairs() {
        let coupling = Coupling::truncated(groups(&["a", "b"]));
        assert!(!coupling.is_all_pairs());
        assert!(coupling.couples("a", "a"));
        assert!(!coupling.couples("a", "b"));

        // Two segments, two filaments each: 10 pairs in all, the 4 cross
        // pairs dropped.
        let mask = coupling.pair_mask(&[2, 2]).unwrap();
        assert_eq!(mask.total_pairs(), 10);
        assert_eq!(mask.kept_pairs(), 6);
        assert!(mask.keeps(0, 1));
        assert!(mask.keeps(2, 2));
        assert!(!mask.keeps(0, 2));
        assert!(!mask.keeps(3, 1));
        assert!(!mask.keeps_everything());
    }

    #[test]
    fn declaring_a_pair_keeps_it() {
        let coupling = Coupling::truncated(groups(&["a", "b", "c"])).coupled("a", "b");
        assert!(coupling.couples("a", "b"));
        assert!(coupling.couples("b", "a"));
        assert!(!coupling.couples("a", "c"));
        let mask = coupling.pair_mask(&[1, 1, 1]).unwrap();
        assert!(mask.keeps(0, 1));
        assert!(!mask.keeps(0, 2));
        assert!(!mask.keeps(1, 2));
        assert_eq!(mask.kept_pairs(), 4);
    }

    #[test]
    fn untagged_segments_share_the_default_group() {
        let coupling = Coupling::truncated(groups(&["", "", "b"]));
        let mask = coupling.pair_mask(&[1, 1, 1]).unwrap();
        assert!(mask.keeps(0, 1), "both segments are in the default group");
        assert!(!mask.keeps(1, 2));
        // A shorter tag list leaves the rest in the default group too.
        let short = Coupling::truncated(groups(&["b"]));
        let mask = short.pair_mask(&[1, 1, 1]).unwrap();
        assert!(!mask.keeps(0, 1));
        assert!(mask.keeps(1, 2));
    }

    #[test]
    fn unknown_and_oversized_declarations_are_errors() {
        let coupling = Coupling::truncated(groups(&["a", "b"])).coupled("a", "typo");
        assert_eq!(
            coupling.validate(2),
            Err(CouplingError::UnknownGroup {
                name: "typo".to_owned()
            })
        );
        assert!(coupling.pair_mask(&[1, 1]).is_err());

        let long = Coupling::truncated(groups(&["a", "b", "c"]));
        assert_eq!(
            long.validate(2),
            Err(CouplingError::SegmentCountMismatch {
                expected: 2,
                got: 3
            })
        );
    }

    #[test]
    fn non_transitive_declarations_are_rejected() {
        // a--b and b--c declared, but not a--c: b's two neighbors are
        // reachable through b but not declared coupled to each other.
        let coupling = Coupling::truncated(groups(&["a", "b", "c"]))
            .coupled("a", "b")
            .coupled("b", "c");
        assert_eq!(
            coupling.validate(3),
            Err(CouplingError::NonTransitiveCoupling {
                groups: ["a".to_owned(), "c".to_owned()]
            })
        );
        assert!(coupling.pair_mask(&[1, 1, 1]).is_err());

        // The same three groups, fully declared as a clique, validate.
        let clique = Coupling::truncated(groups(&["a", "b", "c"]))
            .coupled("a", "b")
            .coupled("b", "c")
            .coupled("a", "c");
        assert_eq!(clique.validate(3), Ok(()));

        // A single isolated pair, with a third untouched group, also
        // validates: there is no third group reachable through anything.
        let isolated_pair = Coupling::truncated(groups(&["a", "b", "c"])).coupled("a", "b");
        assert_eq!(isolated_pair.validate(3), Ok(()));
    }

    #[test]
    fn near_truncated_groups_warn_and_distant_ones_do_not() {
        let coupling = Coupling::truncated(groups(&["a", "b"]));
        // Extent of each trace is its length, 1.
        let near = coupling.truncation_warnings(&two_traces(0.25)).unwrap();
        assert_eq!(near.len(), 1);
        assert_eq!(near[0].groups, ["a".to_owned(), "b".to_owned()]);
        assert!((near[0].separation - 0.25).abs() < 1e-12);
        assert!((near[0].extent - 1.0).abs() < 1e-12);
        assert!(near[0].to_string().contains("'a'"));
        assert!(near[0].to_string().contains("truncated"));

        assert!(coupling
            .truncation_warnings(&two_traces(4.0))
            .unwrap()
            .is_empty());
        // Nothing is truncated, so nothing is reported however close.
        assert!(Coupling::all_pairs()
            .truncation_warnings(&two_traces(0.01))
            .unwrap()
            .is_empty());
        assert!(coupling
            .clone()
            .coupled("a", "b")
            .truncation_warnings(&two_traces(0.01))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn the_default_group_is_named_in_warnings() {
        let coupling = Coupling::truncated(groups(&["", "b"]));
        let warnings = coupling.truncation_warnings(&two_traces(0.1)).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].to_string().contains("(default)"),
            "{}",
            warnings[0]
        );
    }

    #[test]
    fn round_trips_through_json() {
        let coupling = Coupling::truncated(groups(&["a", "b"])).coupled("a", "b");
        let text = serde_json::to_string(&coupling).unwrap();
        assert_eq!(
            serde_json::from_str::<Coupling>(&text).unwrap(),
            coupling,
            "{text}"
        );
        // The default serializes to an empty object and survives it.
        let text = serde_json::to_string(&Coupling::all_pairs()).unwrap();
        assert_eq!(text, "{}");
        assert_eq!(
            serde_json::from_str::<Coupling>(&text).unwrap(),
            Coupling::all_pairs()
        );
        assert!(serde_json::from_str::<Coupling>(r#"{"typo": 1}"#).is_err());
    }
}
