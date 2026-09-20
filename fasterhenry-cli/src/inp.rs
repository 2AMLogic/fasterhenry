//! Reader for the FastHenry `.inp` deck format — a public, documented file
//! format — restricted to the M0 subset.
//!
//! # Clean-room note
//!
//! The `.inp` deck format is a published file format, and a file format is
//! not code; this parser is written from the public format description and
//! from self-authored example decks only. No FastHenry or FastCap source,
//! text, or tables were consulted, in keeping with the repository's
//! clean-room rule (see `CONTRIBUTING.md`).
//!
//! # Supported subset (M0)
//!
//! | Line | Meaning |
//! |------|---------|
//! | `.units um\|mm\|cm\|m\|mil` | Length unit for every coordinate and dimension |
//! | `.default <field>=<v> …` | Defaults for later lines: `x`, `y`, `z`, `w`, `h`, `nwinc`, `nhinc`, `sigma` |
//! | `N<name> [x]=<v> [y]=<v> [z]=<v>` | Node; each coordinate falls back to its `.default` |
//! | `E<name> N<a> N<b> [field]=<v> …` | Segment between two nodes; fields as for `.default` minus `x`/`y`/`z` |
//! | `.external N<+> N<-> [name]` | A port: current in at `N<+>`, out at `N<->`, labelled `name` (extension; default `<+>/<->`) |
//! | `.freq fmin=<v> fmax=<v> ndec=<n>` | Frequency sweep in hertz (see below) |
//! | `G<name> x1 y1 z1 x2 y2 z2 t [nx=] [ny=]` | Ground plane: extent, top surface `z`, thickness `t` down, `nx × ny` cells |
//! | `.hole G<name> x1 y1 x2 y2` | Rectangular hole in that plane's footprint |
//! | `.equiv N<a> N<b>` | Electrically join two nodes into one |
//! | `.end` | End of deck (required) |
//!
//! Lines beginning with `*` are comments; a line beginning with `+`
//! continues the previous line. Directives are case-insensitive; node and
//! element names are case-sensitive alphanumeric tokens (`N1`, `Ea3`).
//! `rho=` (resistivity) is not accepted: this engine takes conductivity —
//! use `sigma = 1/rho`.
//!
//! # Semantics
//!
//! * **Units are mandatory.** A deck without an explicit `.units` line is
//!   rejected rather than silently assuming a default, so a missing unit can
//!   never scale a result by a factor of 10 or 100. Lengths (coordinates,
//!   `w`, `h`) scale with the unit; **conductivity is per deck unit** —
//!   `sigma=5.8e4` under `.units mm` is copper (5.8e4 S/mm = 5.8e7 S/m),
//!   exactly as resistivity would carry the unit in the original format.
//! * **`.freq fmin fmax ndec`** samples `ndec` points per decade,
//!   log-spaced: `f(k) = fmin · 10^(k/ndec)` for `k = 0 … n−1`, with
//!   `n = floor(ndec · log10(fmax/fmin)) + 1`; `fmin` is always the first
//!   point and `fmax` is reached when the decades divide evenly.
//!   `fmin = fmax` (any `ndec`) is the single-frequency case; `fmin = 0` is
//!   allowed only there (the DC solve).
//! * **`.equiv a b`** makes `b` an alias of `a`: every reference — declared
//!   before or after the directive — resolves to `a`, and `b`'s node does
//!   not appear in the resulting geometry.
//! * **Planes connect by landing**: a segment or port endpoint that lies
//!   within a plane's footprint and depth is snapped to the nearest live
//!   cell-centre node, wiring the segment into the plane mesh. Declare the
//!   plane before or after the segment — the assembly is order-independent.
//! * Everything else is rejected with an error carrying the line number.
//!   Deck-level problems that belong to no single line (missing `.units`, a
//!   geometry validation failure) report line 0.

use std::collections::HashMap;

use fasterhenry::geometry::{Geometry, Node, NodeId, SegmentDef};
use fasterhenry::mesh::Port;
use fasterhenry::plane::{GroundPlane, Hole};
use fasterhenry::solve::{Discretization, Subdivision};

/// A parsed `.inp` deck: everything [`fasterhenry::solve::solve`] needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Deck {
    /// The deck's `.title` directive, if any.
    pub title: Option<String>,
    /// Nodes and segments, positions in metres.
    pub geometry: Geometry,
    /// Ports from `.external`, in deck order.
    pub ports: Vec<Port>,
    /// Filament subdivisions ([`Discretization::Uniform`] when every segment
    /// agrees, [`Discretization::PerSegment`] as soon as one differs).
    pub discretization: Discretization,
    /// Frequencies in hertz from `.freq`.
    pub frequencies: Vec<f64>,
}

/// A `G` line pending assembly: the plane and the name `.hole` refers to.
#[derive(Clone, Debug)]
struct PlaneSpec {
    name: String,
    plane: GroundPlane,
}

/// A parse error: what went wrong, and on which line (`0` = deck-level).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {message}")]
pub struct ParseError {
    /// The 1-based line number the error was detected on, or 0 for
    /// deck-level problems.
    pub line: usize,
    /// What went wrong.
    pub message: String,
}

fn err(line: usize, message: impl Into<String>) -> ParseError {
    ParseError {
        line,
        message: message.into(),
    }
}

/// The `.units` directive's length unit, as a factor to metres.
fn unit_factor(unit: &str, line: usize) -> Result<f64, ParseError> {
    match unit {
        "um" => Ok(1e-6),
        "mm" => Ok(1e-3),
        "cm" => Ok(1e-2),
        "m" => Ok(1.0),
        "mil" => Ok(25.4e-6),
        other => Err(err(
            line,
            format!("unknown length unit '{other}' (supported: um, mm, cm, m, mil)"),
        )),
    }
}

/// Per-line field defaults set by `.default`; lengths are already scaled to
/// metres when stored, sigma to S/m.
#[derive(Clone, Copy, Debug, Default)]
struct Defaults {
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    w: Option<f64>,
    h: Option<f64>,
    nwinc: Option<usize>,
    nhinc: Option<usize>,
    sigma: Option<f64>,
}

/// A `<field>=<value>` token, keys lowercased.
fn parse_field(token: &str, line: usize) -> Result<(String, String), ParseError> {
    let (key, value) = token.split_once('=').ok_or_else(|| {
        err(
            line,
            format!("expected <field>=<value>, got '{token}' (supported: x, y, z, w, h, nwinc, nhinc, sigma)"),
        )
    })?;
    if key.is_empty() || value.is_empty() {
        return Err(err(line, format!("empty side of '{token}'")));
    }
    Ok((key.to_ascii_lowercase(), value.to_string()))
}

fn parse_number(text: &str, line: usize) -> Result<f64, ParseError> {
    let value = text
        .parse::<f64>()
        .map_err(|_| err(line, format!("'{text}' is not a number")))?;
    if !value.is_finite() {
        return Err(err(line, format!("'{text}' is not finite")));
    }
    Ok(value)
}

fn parse_count(text: &str, what: &str, line: usize) -> Result<usize, ParseError> {
    let value = text.parse::<usize>().map_err(|_| {
        err(
            line,
            format!("'{text}' is not a valid {what} (a whole number ≥ 1)"),
        )
    })?;
    if value < 1 {
        return Err(err(line, format!("{what} must be ≥ 1, got {value}")));
    }
    Ok(value)
}

/// The value side of a `<field>=<value>` pair: a length (scaled), a
/// conductivity (per deck unit, converted to S/m), or a count.
fn parse_value(key: &str, value: &str, unit: f64, line: usize) -> Result<f64, ParseError> {
    match key {
        "sigma" => Ok(parse_number(value, line)? / unit),
        "nwinc" | "nhinc" | "nx" | "ny" => Ok(parse_count(value, key, line)? as f64),
        _ => Ok(parse_number(value, line)? * unit),
    }
}

/// Sets one default field, rejecting unknown keys with the field list.
fn set_default(
    defaults: &mut Defaults,
    key: &str,
    value: f64,
    line: usize,
) -> Result<(), ParseError> {
    match key {
        "x" => defaults.x = Some(value),
        "y" => defaults.y = Some(value),
        "z" => defaults.z = Some(value),
        "w" => defaults.w = Some(value.abs()),
        "h" => defaults.h = Some(value.abs()),
        "nwinc" => defaults.nwinc = Some(value as usize),
        "nhinc" => defaults.nhinc = Some(value as usize),
        "sigma" => defaults.sigma = Some(value),
        "rho" => {
            return Err(err(
                line,
                "'rho' (resistivity) is not accepted; this engine takes conductivity: sigma = 1/rho (per deck unit)",
            ));
        }
        other => {
            return Err(err(
                line,
                format!("unknown field '{other}' (supported: x, y, z, w, h, nwinc, nhinc, sigma)"),
            ));
        }
    }
    Ok(())
}

/// Node names to node slots, with `.equiv` aliases resolved at lookup and
/// aliased-away slots compacted out of the final geometry.
#[derive(Default)]
struct Names {
    ids: HashMap<String, usize>,
    aliases: HashMap<usize, usize>,
}

impl Names {
    fn define(&mut self, name: &str, id: usize, line: usize) -> Result<(), ParseError> {
        if self.ids.contains_key(name) {
            return Err(err(line, format!("duplicate node name '{name}'")));
        }
        self.ids.insert(name.to_string(), id);
        Ok(())
    }

    fn lookup(&self, name: &str, line: usize) -> Result<usize, ParseError> {
        let mut id = *self
            .ids
            .get(name)
            .ok_or_else(|| err(line, format!("unknown node '{name}'")))?;
        while let Some(&target) = self.aliases.get(&id) {
            id = target;
        }
        Ok(id)
    }

    /// The final index of every live slot: aliased-away slots are dropped
    /// and the rest compacted in order.
    fn compaction(&self, slot_count: usize) -> Vec<Option<usize>> {
        let mut live = 0;
        (0..slot_count)
            .map(|slot| {
                if self.aliases.contains_key(&slot) {
                    None
                } else {
                    live += 1;
                    Some(live - 1)
                }
            })
            .collect()
    }
}

/// Parses a whole deck. See the [module documentation](self) for the subset.
pub fn parse(text: &str) -> Result<Deck, ParseError> {
    // Fold `+` continuation lines into their first line, keeping the first
    // line's number for error reporting; `*` comments and blank lines drop.
    let mut lines: Vec<(usize, Vec<&str>)> = Vec::new();
    let mut last_number = 0;
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        last_number = number;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('*') {
            continue;
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if let Some(continuation) = tokens[0].strip_prefix('+') {
            match lines.last_mut() {
                Some((_, previous)) => {
                    if !continuation.is_empty() {
                        previous.push(continuation);
                    }
                    previous.extend_from_slice(&tokens[1..]);
                }
                None => return Err(err(number, "deck starts with a '+' continuation line")),
            }
        } else {
            lines.push((number, tokens));
        }
    }

    let mut title = None;
    let mut unit: Option<f64> = None;
    let mut defaults = Defaults::default();
    let mut names = Names::default();
    let mut positions: Vec<[f64; 3]> = Vec::new();
    let mut segment_defs: Vec<(usize, usize, f64, f64, f64)> = Vec::new();
    let mut subdivisions: Vec<Subdivision> = Vec::new();
    let mut ports: Vec<Port> = Vec::new();
    let mut frequencies: Vec<f64> = Vec::new();
    let mut planes: Vec<PlaneSpec> = Vec::new();
    let mut ended = false;

    for &(number, ref tokens) in &lines {
        if ended {
            return Err(err(number, "content after .end"));
        }
        let head = tokens[0];
        let keyword = head.to_ascii_lowercase();
        let factor = unit.unwrap_or(1.0);

        if let Some(directive) = keyword.strip_prefix('.') {
            match directive {
                "title" => title = Some(tokens[1..].join(" ")),
                "units" => {
                    if tokens.len() != 2 {
                        return Err(err(number, "expected .units <unit>"));
                    }
                    if unit.is_some() {
                        return Err(err(number, "duplicate .units directive"));
                    }
                    unit = Some(unit_factor(tokens[1], number)?);
                }
                "default" => {
                    if unit.is_none() {
                        return Err(err(number, ".default before .units (lengths need a unit)"));
                    }
                    for token in &tokens[1..] {
                        let (key, raw_value) = parse_field(token, number)?;
                        let value = parse_value(&key, &raw_value, factor, number)?;
                        set_default(&mut defaults, &key, value, number)?;
                    }
                }
                "external" => {
                    if tokens.len() != 3 && tokens.len() != 4 {
                        return Err(err(number, "expected .external N<+> N<-> [name]"));
                    }
                    let positive = names.lookup(tokens[1], number)?;
                    let negative = names.lookup(tokens[2], number)?;
                    let name = tokens.get(3).map(|name| name.to_string());
                    ports.push(Port {
                        positive: NodeId(positive),
                        negative: NodeId(negative),
                        name: name.or_else(|| Some(format!("{}/{}", tokens[1], tokens[2]))),
                    });
                }
                "freq" => {
                    if tokens.len() != 4 {
                        return Err(err(number, "expected .freq fmin=<v> fmax=<v> ndec=<n>"));
                    }
                    let (mut fmin, mut fmax, mut ndec) = (None, None, None);
                    for token in &tokens[1..] {
                        let (key, raw_value) = parse_field(token, number)?;
                        match key.as_str() {
                            "fmin" => fmin = Some(parse_number(&raw_value, number)?),
                            "fmax" => fmax = Some(parse_number(&raw_value, number)?),
                            "ndec" => ndec = Some(parse_count(&raw_value, "ndec", number)?),
                            other => {
                                return Err(err(
                                    number,
                                    format!("unknown .freq field '{other}' (supported: fmin, fmax, ndec)"),
                                ));
                            }
                        }
                    }
                    let (fmin, fmax, ndec) = match (fmin, fmax, ndec) {
                        (Some(fmin), Some(fmax), Some(ndec)) => (fmin, fmax, ndec),
                        _ => return Err(err(number, ".freq needs fmin=, fmax= and ndec=")),
                    };
                    frequencies = frequency_sweep(fmin, fmax, ndec, number)?;
                }
                "equiv" => {
                    if tokens.len() != 3 {
                        return Err(err(number, "expected .equiv N<a> N<b>"));
                    }
                    let a = names.lookup(tokens[1], number)?;
                    let b = names.lookup(tokens[2], number)?;
                    if a == b {
                        return Err(err(
                            number,
                            format!(".equiv of node '{}' with itself", tokens[1]),
                        ));
                    }
                    names.aliases.insert(b, a);
                }
                "hole" => {
                    if tokens.len() != 6 {
                        return Err(err(number, "expected .hole G<name> x1 y1 x2 y2"));
                    }
                    let name = tokens[1];
                    let mut bounds = [0.0f64; 4];
                    for (slot, token) in bounds.iter_mut().zip(&tokens[2..]) {
                        *slot = parse_number(token, number)? * factor;
                    }
                    let wanted = name.strip_prefix(['g', 'G']).unwrap_or(name);
                    let plane = planes
                        .iter_mut()
                        .find(|spec| spec.name[1..] == *wanted || spec.name == name)
                        .ok_or_else(|| {
                            err(
                                number,
                                format!("'.hole' names unknown ground plane '{name}'"),
                            )
                        })?;
                    plane.plane.holes.push(Hole {
                        lo: [bounds[0].min(bounds[2]), bounds[1].min(bounds[3])],
                        hi: [bounds[0].max(bounds[2]), bounds[1].max(bounds[3])],
                    });
                }
                "end" => {
                    if tokens.len() != 1 {
                        return Err(err(number, "expected .end (no arguments)"));
                    }
                    ended = true;
                }
                other => {
                    return Err(err(
                        number,
                        format!(
                            "'.{other}' is not in the deck subset (the rest of FastHenry is M1+)"
                        ),
                    ));
                }
            }
            continue;
        }

        match keyword.chars().next() {
            Some('n') => {
                if unit.is_none() {
                    return Err(err(number, "node before .units (lengths need a unit)"));
                }
                let mut coordinates = [defaults.x, defaults.y, defaults.z];
                for token in &tokens[1..] {
                    let (key, raw_value) = parse_field(token, number)?;
                    let value = parse_value(&key, &raw_value, factor, number)?;
                    match key.as_str() {
                        "x" => coordinates[0] = Some(value),
                        "y" => coordinates[1] = Some(value),
                        "z" => coordinates[2] = Some(value),
                        other => {
                            return Err(err(
                                number,
                                format!("unknown node field '{other}' (supported: x, y, z)"),
                            ));
                        }
                    }
                }
                let position = match coordinates {
                    [Some(x), Some(y), Some(z)] => [x, y, z],
                    _ => {
                        return Err(err(
                            number,
                            format!(
                                "node '{head}': a coordinate is unset and no .default covers it (set it, or .default x=… y=… z=…)"
                            ),
                        ));
                    }
                };
                positions.push(position);
                names.define(head, positions.len() - 1, number)?;
            }
            Some('e') => {
                if unit.is_none() {
                    return Err(err(number, "element before .units (lengths need a unit)"));
                }
                if tokens.len() < 3 {
                    return Err(err(number, "expected E<name> N<a> N<b> [field]=<value> …"));
                }
                let a = names.lookup(tokens[1], number)?;
                let b = names.lookup(tokens[2], number)?;
                let mut w = defaults.w;
                let mut h = defaults.h;
                let mut nwinc = defaults.nwinc;
                let mut nhinc = defaults.nhinc;
                let mut sigma = defaults.sigma;
                for token in &tokens[3..] {
                    let (key, raw_value) = parse_field(token, number)?;
                    let value = parse_value(&key, &raw_value, factor, number)?;
                    match key.as_str() {
                        "w" => w = Some(value.abs()),
                        "h" => h = Some(value.abs()),
                        "nwinc" => nwinc = Some(value as usize),
                        "nhinc" => nhinc = Some(value as usize),
                        "sigma" => sigma = Some(value),
                        "x" | "y" | "z" => {
                            return Err(err(
                                number,
                                format!("'{key}' is a node field, not a segment field"),
                            ));
                        }
                        "rho" => {
                            return Err(err(
                                number,
                                "'rho' (resistivity) is not accepted; this engine takes conductivity: sigma = 1/rho (per deck unit)",
                            ));
                        }
                        other => {
                            return Err(err(
                                number,
                                format!("unknown field '{other}' (supported: w, h, nwinc, nhinc, sigma)"),
                            ));
                        }
                    }
                }
                let (w, h, sigma) = match (w, h, sigma) {
                    (Some(w), Some(h), Some(sigma)) => (w, h, sigma),
                    (None, _, _) => {
                        return Err(err(
                            number,
                            format!("segment '{head}' has no width: set w= here or in .default"),
                        ));
                    }
                    (_, None, _) => {
                        return Err(err(
                            number,
                            format!("segment '{head}' has no height: set h= here or in .default"),
                        ));
                    }
                    (_, _, None) => {
                        return Err(err(
                            number,
                            format!("segment '{head}' has no conductivity: set sigma= here or in .default"),
                        ));
                    }
                };
                segment_defs.push((a, b, w, h, sigma));
                subdivisions.push(Subdivision::new(nwinc.unwrap_or(1), nhinc.unwrap_or(1)));
            }
            Some('g') => {
                if unit.is_none() {
                    return Err(err(
                        number,
                        "ground plane before .units (lengths need a unit)",
                    ));
                }
                if tokens.len() < 8 {
                    return Err(err(
                        number,
                        "expected G<name> x1 y1 z1 x2 y2 z2 thickness [nx=…] [ny=…]",
                    ));
                }
                let mut corners = [None; 7];
                for (slot, token) in corners.iter_mut().zip(&tokens[1..8]) {
                    *slot = Some(
                        parse_number(token, number).map_err(|mut error| {
                            error.line = number;
                            error
                        })? * factor,
                    );
                }
                let mut nx = 1usize;
                let mut ny = 1usize;
                for token in &tokens[8..] {
                    let (key, raw_value) = parse_field(token, number)?;
                    let value = parse_value(&key, &raw_value, factor, number)?;
                    match key.as_str() {
                        "nx" => nx = value as usize,
                        "ny" => ny = value as usize,
                        other => {
                            return Err(err(
                                number,
                                format!("unknown G field '{other}' (supported: nx, ny)"),
                            ));
                        }
                    }
                }
                let corners: Vec<f64> = corners.into_iter().flatten().collect();
                let [x1, y1, z1, x2, y2, z2, thickness] = [
                    corners[0], corners[1], corners[2], corners[3], corners[4], corners[5],
                    corners[6],
                ];
                if nx < 1 || ny < 1 {
                    return Err(err(
                        number,
                        format!(
                            "ground plane '{head}' needs nx and ny >= 1 (got nx={nx}, ny={ny})"
                        ),
                    ));
                }
                planes.push(PlaneSpec {
                    name: head.to_string(),
                    plane: GroundPlane {
                        lo: [x1.min(x2), y1.min(y2)],
                        hi: [x1.max(x2), y1.max(y2)],
                        z_top: z1.max(z2),
                        thickness,
                        nx,
                        ny,
                        sigma: defaults.sigma.ok_or_else(|| {
                            err(
                                number,
                                format!(
                                    "ground plane '{head}' has no conductivity: set sigma= in .default"
                                ),
                            )
                        })?,
                        holes: Vec::new(),
                    },
                });
            }
            _ => {
                return Err(err(
                    number,
                    format!("unrecognized line '{head}' (expected N…, E…, or a .directive)"),
                ));
            }
        }
    }

    if !ended {
        return Err(err(last_number, "deck has no .end directive"));
    }
    if unit.is_none() {
        return Err(err(
            last_number,
            "deck has no .units directive (units are mandatory: .units um|mm|cm|m|mil)",
        ));
    }
    if ports.is_empty() {
        return Err(err(0, "deck has no .external port"));
    }
    if frequencies.is_empty() {
        return Err(err(0, "deck has no .freq sweep"));
    }

    // Apply .equiv: compact aliased-away nodes out of the geometry and
    // remap every reference. Resolution follows the alias chain, so a
    // reference declared *before* the .equiv directive lands on the
    // canonical node too — the deck is one circuit, not a timeline.
    let compaction = names.compaction(positions.len());
    let resolve = |slot: usize| -> usize {
        let mut id = slot;
        while let Some(&target) = names.aliases.get(&id) {
            id = target;
        }
        compaction[id].expect("a canonical node is live by construction")
    };
    // Assembly: ground-plane meshes first (their cell nodes and bars), then
    // the declared nodes, then the segments — with every endpoint that
    // lands in a plane's footprint snapped to the nearest live cell node.
    let mut geometry = Geometry::new();
    let mut plane_meshes: Vec<(&PlaneSpec, Vec<Vec<Option<NodeId>>>)> = Vec::new();
    let mut plane_bars = 0usize;
    for spec in &planes {
        let centres = spec
            .plane
            .build_into(&mut geometry)
            .map_err(|error| ParseError {
                line: 0,
                message: error.to_string(),
            })?;
        plane_bars = geometry.segment_count();
        plane_meshes.push((spec, centres));
    }
    // Endpoint resolution: follow `.equiv` aliases to the canonical slot,
    // take that slot's position, and if it lands in a plane's footprint,
    // snap to the nearest live cell node. Plane ids are final here; the
    // declared nodes that are still referenced as themselves get their ids
    // below (a snapped endpoint's declared node is dropped, not orphaned).
    let snap = |slot: usize| -> Result<Option<usize>, ParseError> {
        let live = resolve(slot);
        let position = live_position(live, &positions, &compaction);
        for (spec, centres) in &plane_meshes {
            if spec.plane.contains(position, 0.0) {
                return Ok(Some(
                    spec.plane
                        .attach(centres, position)
                        .map_err(|error| ParseError {
                            line: 0,
                            message: error.to_string(),
                        })?
                        .0,
                ));
            }
        }
        Ok(None)
    };

    // One endpoint: the plane node it snapped to (final id), or the live
    // declared slot it stays on (id assigned below).
    let endpoint = |slot: usize| -> Result<(Option<usize>, usize), ParseError> {
        Ok((snap(slot)?, resolve(slot)))
    };
    type End = (Option<usize>, usize);
    let segment_ends: Vec<(End, End, (f64, f64, f64))> = segment_defs
        .iter()
        .map(|&(a, b, w, h, sigma)| Ok((endpoint(a)?, endpoint(b)?, (w, h, sigma))))
        .collect::<Result<_, ParseError>>()?;
    let port_ends: Vec<(End, End)> = ports
        .iter()
        .map(|port| Ok((endpoint(port.positive.0)?, endpoint(port.negative.0)?)))
        .collect::<Result<_, ParseError>>()?;

    // Live declared slots still referenced as themselves keep their nodes.
    let mut used = vec![false; positions.len()];
    let mark = |endpoint: &(Option<usize>, usize), used: &mut Vec<bool>| {
        if endpoint.0.is_none() {
            used[endpoint.1] = true;
        }
    };
    for (a, b, _) in &segment_ends {
        mark(a, &mut used);
        mark(b, &mut used);
    }
    for (a, b) in &port_ends {
        mark(a, &mut used);
        mark(b, &mut used);
    }
    let mut live_to_id = vec![usize::MAX; positions.len()];
    let mut next_id = geometry.nodes().len();
    for (slot, &position) in positions.iter().enumerate() {
        let Some(live) = compaction[slot] else {
            continue; // aliased away by .equiv
        };
        if !used[live] {
            continue; // every reference snapped into a plane
        }
        geometry
            .add_node(Node::new(position[0], position[1], position[2]))
            .map_err(|error| ParseError {
                line: 0,
                message: error.to_string(),
            })?;
        live_to_id[live] = next_id;
        next_id += 1;
    }

    let final_id = |endpoint: &(Option<usize>, usize)| -> usize {
        endpoint.0.unwrap_or(live_to_id[endpoint.1])
    };
    for ((a, b, (w, h, sigma)), _) in segment_ends.iter().zip(0..) {
        geometry
            .add_segment(SegmentDef::new(
                NodeId(final_id(a)),
                NodeId(final_id(b)),
                *w,
                *h,
                *sigma,
            ))
            .map_err(|error| ParseError {
                line: 0,
                message: error.to_string(),
            })?;
    }
    for (port, (positive, negative)) in ports.iter_mut().zip(&port_ends) {
        port.positive = NodeId(final_id(positive));
        port.negative = NodeId(final_id(negative));
    }

    let discretization = {
        let mut all = vec![Subdivision::SINGLE; plane_bars];
        all.extend(subdivisions);
        let subdivisions = all;
        let first = subdivisions[0];
        if subdivisions.iter().all(|&sub| sub == first) {
            Discretization::Uniform(first)
        } else {
            Discretization::PerSegment(subdivisions)
        }
    };

    Ok(Deck {
        title,
        geometry,
        ports,
        discretization,
        frequencies,
    })
}

/// The position of the `live`-th surviving slot (the index space `resolve`
/// produces: aliased-away slots are compacted out, survivors keep order).
fn live_position(live: usize, positions: &[[f64; 3]], compaction: &[Option<usize>]) -> [f64; 3] {
    let mut seen = 0usize;
    for (slot, mapped) in compaction.iter().enumerate() {
        if mapped.is_some() {
            if seen == live {
                return positions[slot];
            }
            seen += 1;
        }
    }
    unreachable!("compaction guarantees a live slot for every resolved id")
}

/// The `.freq` decade sweep; see the module documentation. Integer decades
/// use exact `powi` so round decimal frequencies round-trip bitwise.
fn frequency_sweep(fmin: f64, fmax: f64, ndec: usize, line: usize) -> Result<Vec<f64>, ParseError> {
    if !(fmin.is_finite() && fmin >= 0.0 && fmax.is_finite() && fmax >= 0.0) {
        return Err(err(line, "frequencies must be finite and ≥ 0"));
    }
    if fmax < fmin {
        return Err(err(line, format!("fmax ({fmax}) is below fmin ({fmin})")));
    }
    if fmin == fmax {
        return Ok(vec![fmin]);
    }
    if fmin == 0.0 {
        return Err(err(
            line,
            "fmin = 0 is only allowed as the single-frequency case (.freq fmin=0 fmax=0 runs the DC solve)",
        ));
    }
    let decades = fmax.log10() - fmin.log10();
    let count = (decades * ndec as f64 + 1.0 + 1e-9).floor() as usize;
    Ok((0..count)
        .map(|k| {
            let (quotient, remainder) = (k / ndec, k % ndec);
            if remainder == 0 {
                fmin * 10f64.powi(quotient as i32)
            } else {
                fmin * 10f64.powf(k as f64 / ndec as f64)
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(text: &str) -> Deck {
        parse(text).unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn minimal_deck_parses() {
        let deck = parse_ok(
            "\
.title minimal
.units m
n1 x=0 y=0 z=0
n2 x=1e-3 y=0 z=0
e1 n1 n2 w=1e-3 h=1e-4 sigma=5.8e7
.external n1 n2
.freq fmin=1e3 fmax=1e6 ndec=10
.end
",
        );
        assert_eq!(deck.title.as_deref(), Some("minimal"));
        assert_eq!(deck.geometry.nodes().len(), 2);
        assert_eq!(deck.geometry.segment_count(), 1);
        assert_eq!(deck.ports.len(), 1);
        assert_eq!(deck.ports[0].name.as_deref(), Some("n1/n2"));
        assert_eq!(deck.frequencies.len(), 31);
        assert_eq!(deck.frequencies[0], 1e3);
        assert_eq!(deck.frequencies[30], 1e6);
        assert_eq!(
            deck.discretization,
            Discretization::Uniform(Subdivision::SINGLE)
        );
    }

    #[test]
    fn units_scale_lengths_and_conductivity() {
        let deck = parse_ok(
            "\
.units um
n1 x=0 y=0 z=0
n2 x=1000 y=0 z=0
e1 n1 n2 w=100 h=10 sigma=1.0
.external n1 n2
.freq fmin=1 fmax=1 ndec=1
.end
",
        );
        let segment = deck.geometry.segment(0).unwrap();
        assert!((segment.length() - 1e-3).abs() < 1e-18);
        assert!((segment.width - 100e-6).abs() < 1e-18);
        // sigma=1.0 per um: 1.0 / 1e-6 = 1e6 S/m.
        assert!((segment.sigma - 1e6).abs() < 1e-9);
    }

    #[test]
    fn copper_in_mm_units() {
        let deck = parse_ok(
            "\
.units mm
.default z=0 sigma=5.8e4
n1 x=0 y=0
n2 x=10 y=0
e1 n1 n2 w=0.2 h=0.035
.external n1 n2
.freq fmin=1e6 fmax=1e6 ndec=1
.end
",
        );
        let segment = deck.geometry.segment(0).unwrap();
        assert!((segment.sigma - 5.8e7).abs() < 5.8e7 * 1e-15);
        assert_eq!(segment.a.z, 0.0);
    }

    #[test]
    fn defaults_and_continuation_lines() {
        let deck = parse_ok(
            "\
.units mm
* comment line
.default z=0 w=0.1 h=0.01 sigma=5.8e4 nhinc=2
n1 x=0 y=0
n2 x=10 y=0
e1 n1 n2 nwinc=3
+ sigma=1.0
.external n1 n2
.freq fmin=1e6 fmax=1e6 ndec=1
.end
",
        );
        let segment = deck.geometry.segment(0).unwrap();
        assert!((segment.width - 0.1e-3).abs() < 1e-18);
        assert!((segment.sigma - 1.0 / 1e-3).abs() < 1e-12);
        assert_eq!(
            deck.discretization,
            Discretization::Uniform(Subdivision::new(3, 2))
        );
    }

    #[test]
    fn equiv_compacts_the_aliased_node_away() {
        let deck = parse_ok(
            "\
.units m
n1 x=0 y=0 z=0
n1b x=5 y=5 z=5
n2 x=1e-3 y=0 z=0
e1 n1b n2 w=1e-3 h=1e-3 sigma=1.0
.equiv n1 n1b
e2 n1b n2 w=1e-3 h=1e-3 sigma=1.0
.external n1 n2
.freq fmin=1 fmax=1 ndec=1
.end
",
        );
        // The aliased node disappears; every reference — declared before or
        // after .equiv — resolves to the canonical node n1 at the origin.
        assert_eq!(deck.geometry.nodes().len(), 2);
        assert_eq!(
            deck.geometry.segment(0).unwrap().a,
            Node::new(0e0, 0e0, 0e0)
        );
        assert_eq!(
            deck.geometry.segment(1).unwrap().a,
            Node::new(0e0, 0e0, 0e0)
        );
    }

    #[test]
    fn external_accepts_a_port_name() {
        let deck = parse_ok(
            "\
.units m
n1 x=0 y=0 z=0
n2 x=1 y=0 z=0
e1 n1 n2 w=1 h=1 sigma=1.0
.external n1 n2 trace
.freq fmin=1 fmax=1 ndec=1
.end
",
        );
        assert_eq!(deck.ports[0].name.as_deref(), Some("trace"));
    }

    #[test]
    fn dc_single_frequency() {
        let deck = parse_ok(
            "\
.units m
n1 x=0 y=0 z=0
n2 x=1 y=0 z=0
e1 n1 n2 w=1 h=1 sigma=1.0
.external n1 n2
.freq fmin=0 fmax=0 ndec=1
.end
",
        );
        assert_eq!(deck.frequencies, vec![0.0]);
    }

    #[test]
    fn g_ground_plane_builds_mesh_and_holes() {
        let deck = parse_ok(
            "\
.units mm
.default sigma=5.8e4
Gp 0 0 0 10 6 0 0.035 nx=5 ny=3
.hole Gp 4.9 2.9 5.1 3.1
n1 x=1 y=0 z=0
n2 x=9 y=0 z=0
e1 n1 n2 w=0.2 h=0.035
.external n1 n2
.freq fmin=1 fmax=1 ndec=1
.end
",
        );
        // 15 cells minus the holed one; 22 bars minus its 4 incident,
        // plus the declared segment.
        assert_eq!(deck.geometry.nodes().len(), 14);
        assert_eq!(deck.geometry.segment_count(), 18 + 1);
        // The segment endpoints land in the plane footprint and are
        // snapped off the declared nodes onto cell centres.
        let end_a = deck.geometry.segment(18).unwrap().a;
        assert_ne!(end_a, Node::new(1e-3, 0.0, 0.0));
    }

    #[test]
    fn g_plane_errors_carry_lines() {
        let error = parse(
            "\
.units mm
.default sigma=5.8e4
.hole Gq 0 0 1 1
.end
",
        )
        .unwrap_err();
        assert!(error.message.contains("unknown ground plane 'Gq'"));
        let error = parse(
            "\
.units mm
Gp 0 0 0 10 6 0 0.035 nx=0
.end
",
        )
        .unwrap_err();
        assert!(error.message.contains("nx"));
        let error = parse(
            "\
.units mm
Gp 0 0 0 10 6 0 0.035
.end
",
        )
        .unwrap_err();
        assert!(error.message.contains("conductivity"));
    }

    #[test]
    fn missing_units_missing_end_and_unknown_directive() {
        let error = parse("n1 x=0 y=0 z=0\n.end\n").unwrap_err();
        assert!(error.message.contains(".units"));
        let valid_prefix = "\
.units m
n1 x=0 y=0 z=0
n2 x=1 y=0 z=0
e1 n1 n2 w=1 h=1 sigma=1
.external n1 n2
.freq fmin=1 fmax=1 ndec=1
";
        let error = parse(valid_prefix).unwrap_err();
        assert!(error.message.contains(".end"));
        let error = parse(".units m\n.cparams tol=1e-3\n.end\n").unwrap_err();
        assert_eq!(error.line, 2);
        assert!(error.message.contains("deck subset"));
    }

    #[test]
    fn rho_gets_a_conversion_hint() {
        let error = parse(
            "\
.units m
n1 x=0 y=0 z=0
n2 x=1 y=0 z=0
e1 n1 n2 w=1 h=1 rho=1.7e-8
.external n1 n2
.freq fmin=1 fmax=1 ndec=1
.end
",
        )
        .unwrap_err();
        assert_eq!(error.line, 4);
        assert!(error.message.contains("sigma = 1/rho"));
    }

    #[test]
    fn line_numbers_count_comments_and_blanks() {
        let error = parse(
            "\
* leading comment

.units m
n1 x=0 y=0 z=0
n2 x=1 y=0 z=0
e1 n1 n9 w=1 h=1 sigma=1
.external n1 n2
.freq fmin=1 fmax=1 ndec=1
.end
",
        )
        .unwrap_err();
        assert_eq!(error.line, 6);
        assert!(error.message.contains("n9"));
    }

    #[test]
    fn missing_field_names_the_segment_and_the_field() {
        let error = parse(
            "\
.units m
n1 x=0 y=0 z=0
n2 x=1 y=0 z=0
e1 n1 n2 h=1 sigma=1
.external n1 n2
.freq fmin=1 fmax=1 ndec=1
.end
",
        )
        .unwrap_err();
        assert_eq!(error.line, 4);
        assert!(error.message.contains("'e1'"));
        assert!(error.message.contains("width"));
    }

    #[test]
    fn unset_node_coordinate_points_at_the_default() {
        let error = parse(
            "\
.units m
n1 x=0
n2 x=1 y=0 z=0
e1 n1 n2 w=1 h=1 sigma=1
.external n1 n2
.freq fmin=1 fmax=1 ndec=1
.end
",
        )
        .unwrap_err();
        assert_eq!(error.line, 2);
        assert!(error.message.contains(".default"));
    }

    #[test]
    fn freq_validation() {
        let template = "\
.units m
n1 x=0 y=0 z=0
n2 x=1 y=0 z=0
e1 n1 n2 w=1 h=1 sigma=1
.external n1 n2
.freq {SWEEP}
.end
";
        for (sweep, what) in [
            ("fmin=1e6 fmax=1e3 ndec=10", "fmax < fmin"),
            ("fmin=0 fmax=1e6 ndec=10", "log sweep from zero"),
            ("fmin=-1 fmax=1e6 ndec=10", "negative"),
            ("fmin=1 fmax=1e6", "missing ndec"),
        ] {
            let error = parse(&template.replace("{SWEEP}", sweep)).unwrap_err();
            assert_eq!(error.line, 6, "{what}: {}", error.message);
        }
    }

    #[test]
    fn external_needs_known_nodes() {
        let error = parse(
            "\
.units m
n1 x=0 y=0 z=0
n2 x=1 y=0 z=0
e1 n1 n2 w=1 h=1 sigma=1
.external n1 n7
.freq fmin=1 fmax=1 ndec=1
.end
",
        )
        .unwrap_err();
        assert_eq!(error.line, 5);
        assert!(error.message.contains("n7"));
    }

    #[test]
    fn case_insensitive_directives_and_field_keys() {
        let deck = parse_ok(
            "\
.UNITS m
n1 X=0 Y=0 Z=0
n2 x=1 y=0 z=0
e1 n1 n2 W=1 H=1 SIGMA=1
.External n1 n2
.FREQ FMIN=1 FMAX=1 NDEC=1
.END
",
        );
        assert_eq!(deck.geometry.segment_count(), 1);
        assert_eq!(deck.ports.len(), 1);
    }

    #[test]
    fn decade_frequencies_hit_round_values_exactly() {
        let frequencies = frequency_sweep(1e6, 1e9, 1, 6).unwrap();
        assert_eq!(frequencies, vec![1e6, 1e7, 1e8, 1e9]);
        let frequencies = frequency_sweep(1e3, 1e6, 10, 6).unwrap();
        assert_eq!(frequencies.len(), 31);
        assert_eq!(frequencies[30], 1e6);
    }
}
