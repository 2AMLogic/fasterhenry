//! The JSON problem document: geometry, ports, discretization, and the
//! frequency sweep that [`crate::run`] needs — independent of whether it was
//! read from a `.inp` deck or a JSON document directly.

use fasterhenry::coupling::Coupling;
use fasterhenry::geometry::Geometry;
use fasterhenry::mesh::Port;
use fasterhenry::solve::Discretization;
use serde::{Deserialize, Serialize};

use crate::inp::Deck;

/// A complete input to [`crate::run`]: geometry, ports, filament
/// discretization, and the frequency sweep, all in SI units (metres,
/// siemens/metre, hertz).
///
/// This is exactly the shape a JSON problem document deserializes to
/// (unknown fields are rejected, matching [`Geometry`]'s own strictness); a
/// `.inp` deck converts into one via [`From<Deck>`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Problem {
    /// Nodes and segments.
    pub geometry: Geometry,
    /// Ports at which the impedance is extracted, in the order of the rows
    /// and columns of the result.
    pub ports: Vec<Port>,
    /// How each segment is cut into filaments.
    pub discretization: Discretization,
    /// Which conductor groups are magnetically coupled. Absent — the default
    /// — couples every pair, so an older document reads unchanged.
    #[serde(default, skip_serializing_if = "Coupling::is_trivial")]
    pub coupling: Coupling,
    /// The frequency sweep, in hertz.
    pub frequencies_hz: Vec<f64>,
}

impl From<Deck> for Problem {
    fn from(deck: Deck) -> Self {
        Self {
            geometry: deck.geometry,
            ports: deck.ports,
            discretization: deck.discretization,
            coupling: deck.coupling,
            frequencies_hz: deck.frequencies,
        }
    }
}

#[cfg(test)]
mod tests {
    use fasterhenry::geometry::{Node, NodeId, SegmentDef};
    use fasterhenry::solve::Subdivision;

    use super::*;

    fn sample() -> Problem {
        Problem {
            geometry: Geometry::from_parts(
                vec![Node::new(0.0, 0.0, 0.0), Node::new(1e-3, 0.0, 0.0)],
                vec![SegmentDef::new(NodeId(0), NodeId(1), 1e-4, 3.5e-5, 5.8e7)],
            )
            .unwrap(),
            ports: vec![Port::new(NodeId(0), NodeId(1)).named("trace")],
            discretization: Discretization::Uniform(Subdivision::new(3, 2)),
            coupling: Coupling::all_pairs(),
            frequencies_hz: vec![0.0, 1e6],
        }
    }

    #[test]
    fn round_trips_through_json() {
        let problem = sample();
        let text = serde_json::to_string(&problem).unwrap();
        let parsed: Problem = serde_json::from_str(&text).unwrap();
        assert_eq!(problem, parsed);
    }

    /// A truncating coupling survives the document; the default one is left
    /// out of it entirely, so documents written before the knob existed read
    /// back identically.
    #[test]
    fn coupling_round_trips_and_the_default_is_omitted() {
        let text = serde_json::to_string(&sample()).unwrap();
        assert!(!text.contains("coupling"), "{text}");

        let mut problem = sample();
        problem.coupling = Coupling::truncated(vec!["board".to_owned()]);
        let text = serde_json::to_string(&problem).unwrap();
        assert!(text.contains("coupling"), "{text}");
        assert_eq!(serde_json::from_str::<Problem>(&text).unwrap(), problem);
    }

    #[test]
    fn unknown_field_is_rejected() {
        let mut value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&sample()).unwrap()).unwrap();
        value["typo"] = serde_json::json!(true);
        let error = serde_json::from_value::<Problem>(value).unwrap_err();
        assert!(error.to_string().contains("typo"));
    }
}
