//! Geometry and assembly regressions for unusable surface grading (#33).

use fasterhenry::{
    discretize_graded, solve, Discretization, Geometry, MeshSystem, Node, Port, Segment,
    SegmentDef, SkinDepthGrading, SolveError, MU0,
};
use nalgebra::{Rotation3, Unit, Vector3};

const COPPER: f64 = 5.8e7;

fn trace(scale: f64, translation: [f64; 3]) -> Segment {
    Segment::new(
        Node::from(translation),
        Node::new(
            translation[0] + 0.05 * scale,
            translation[1],
            translation[2],
        ),
        0.005 * scale,
        200e-6 * scale,
        COPPER,
    )
}

fn geometry(segment: &Segment) -> (Geometry, Vec<Port>) {
    let mut geometry = Geometry::new();
    let a = geometry.add_node(segment.a).unwrap();
    let b = geometry.add_node(segment.b).unwrap();
    geometry
        .add_segment(SegmentDef::new(
            a,
            b,
            segment.width,
            segment.height,
            segment.sigma,
        ))
        .unwrap();
    (geometry, vec![Port::new(a, b)])
}

#[test]
fn pathological_surface_cells_are_rejected_before_kernel_evaluation() {
    for scale in [1e-6, 1.0, 1e6] {
        for nw in [1, 8] {
            assert!(
                discretize_graded(&trace(scale, [0.0; 3]), nw, 56, 4.0).is_err(),
                "{nw} x 56 at 4:1 must fail at scale {scale}"
            );
        }
    }
}

#[test]
fn reported_nonpassive_impedances_fail_explicitly() {
    let segment = trace(1.0, [0.0; 3]);
    let (geometry, ports) = geometry(&segment);
    // Original reproduction: t / delta = 10 on a 50 mm x 5 mm x 200 um trace.
    let delta = segment.height / 10.0;
    let frequency = 1.0 / (std::f64::consts::PI * MU0 * COPPER * delta * delta);
    for nw in [1, 8] {
        let error = solve(
            &geometry,
            &ports,
            &Discretization::graded(nw, 56, 4.0),
            &[frequency],
        )
        .unwrap_err();
        assert!(matches!(error, SolveError::Discretize { segment: 0, .. }));
        let diagnostic = error.to_string();
        for expected in ["segment 0", "height", &format!("{nw} x 56"), "ratio 4"] {
            assert!(diagnostic.contains(expected), "{diagnostic}");
        }
    }
}

#[test]
fn automatic_grading_validates_the_selected_grid() {
    let (geometry, ports) = geometry(&trace(1.0, [0.0; 3]));
    let grading =
        Discretization::SkinDepth(SkinDepthGrading::new(1e50, 0.5, 4.0).with_max_per_axis(56));
    let error = MeshSystem::assemble(&geometry, &ports, &grading).unwrap_err();
    let diagnostic = error.to_string();
    for expected in ["segment 0", "width", "56 x 56", "ratio 4"] {
        assert!(diagnostic.contains(expected), "{diagnostic}");
    }
}

#[test]
fn underflowed_skin_depth_does_not_fall_back_to_one_filament() {
    let (geometry, ports) = geometry(&trace(1.0, [0.0; 3]));
    // The finite frequency overflows the product in delta, making delta zero.
    let error = MeshSystem::assemble(
        &geometry,
        &ports,
        &Discretization::skin_depth(1e308, 0.5, 4.0),
    )
    .unwrap_err();
    assert!(error.to_string().contains("32 x 32"));
}

#[test]
fn diagnostic_identifies_the_later_failing_segment() {
    let mut geometry = Geometry::new();
    let a = geometry.add_node(Node::new(0.0, 0.0, 0.0)).unwrap();
    let b = geometry.add_node(Node::new(0.05, 0.0, 0.0)).unwrap();
    let c = geometry.add_node(Node::new(0.1, 0.0, 0.0)).unwrap();
    geometry
        .add_segment(SegmentDef::new(a, b, 0.05, 0.05, COPPER))
        .unwrap();
    geometry
        .add_segment(SegmentDef::new(b, c, 0.005, 200e-6, COPPER))
        .unwrap();
    let error = MeshSystem::assemble(
        &geometry,
        &[Port::new(a, c)],
        &Discretization::graded(1, 30, 2.0),
    )
    .unwrap_err();
    for expected in ["segment 1", "height", "1 x 30", "ratio 2"] {
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn useful_grids_remain_valid_under_scaling_and_translation() {
    for scale in [1e-6, 1.0, 1e6] {
        let segment = trace(scale, [2.0 * scale, -3.0 * scale, 4.0 * scale]);
        let filaments = discretize_graded(&segment, 1, 16, 2.0).unwrap();
        assert_eq!(filaments.len(), 16);
        let area: f64 = filaments.iter().map(|f| f.area()).sum();
        assert!((area / segment.area() - 1.0).abs() < 1e-12);
    }
}

#[test]
fn translated_coordinates_cannot_collapse_cell_offsets() {
    // Width/height offsets are lost at this origin, even with modest grading.
    let error = discretize_graded(&trace(1.0, [0.0, 1e15, 1e15]), 4, 4, 2.0).unwrap_err();
    assert!(error.to_string().contains("width"));
    // An irrelevant large axial coordinate does not reduce transverse precision.
    assert!(discretize_graded(&trace(1.0, [1e9, 0.0, 0.0]), 1, 16, 2.0).is_ok());
}

#[test]
fn rotated_coordinate_cancellation_does_not_hide_unrepresentable_cells() {
    let base = trace(1.0, [0.0; 3]);
    let rotation =
        Rotation3::from_axis_angle(&Unit::new_normalize(Vector3::new(1.0, 2.0, 3.0)), 0.8);
    for (distance, supported) in [(1.0, true), (1e9, false)] {
        // Translation along length projects to zero on both transverse axes
        // mathematically, but all world-coordinate components still round.
        let shift = rotation * Vector3::new(distance, 0.0, 0.0);
        let moved = Segment {
            a: Node::from(rotation * base.a.position() + shift),
            b: Node::from(rotation * base.b.position() + shift),
            width_dir: Some((rotation * Vector3::y()).into()),
            ..base
        };
        assert_eq!(discretize_graded(&moved, 1, 16, 2.0).is_ok(), supported);
    }
}

#[test]
fn unit_ratio_grading_still_checks_coordinate_resolution() {
    let segment = trace(1.0, [0.0, 1e15, 1e15]);
    assert!(discretize_graded(&segment, 4, 4, 1.0).is_err());
    let (geometry, ports) = geometry(&segment);
    assert!(MeshSystem::assemble(&geometry, &ports, &Discretization::graded(4, 4, 1.0)).is_err());
}

#[test]
fn representable_cells_outside_kernel_conditioning_range_are_rejected() {
    // These cells are distinct in f64, but the longitudinal aspect exceeds 1e7.
    let segment = trace(1.0, [0.0; 3]);
    assert!(discretize_graded(&segment, 1, 30, 2.0).is_err());
    assert!(discretize_graded(&segment, 1, 24, 2.0).is_ok());
}

#[test]
fn wide_short_segments_also_bound_cross_section_conditioning() {
    let segment = Segment::new(
        Node::new(0.0, 0.0, 0.0),
        Node::new(1e-3, 0.0, 0.0),
        0.1,
        1e-4,
        COPPER,
    );
    assert!(discretize_graded(&segment, 1, 30, 2.0).is_err());
}

#[test]
fn overflowing_weight_sum_cannot_create_zero_sized_cells() {
    let error = discretize_graded(&trace(1.0, [0.0; 3]), 4, 1, f64::MAX).unwrap_err();
    assert!(error.to_string().contains("width"));
}
