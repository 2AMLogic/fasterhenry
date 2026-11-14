//! `docs/input-format.md` documents the JSON problem document. Every
//! complete example in it is extracted from the Markdown and parsed as a
//! [`Problem`], so the document cannot drift away from the schema it
//! describes.
//!
//! The convention the extraction relies on: a ```` ```json ```` fence in that
//! file is always a *complete* problem document. Field fragments are written
//! inline, in tables, never as a fenced `json` block.

use std::path::PathBuf;

use fasterhenry_cli::{run, Problem};

fn document() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/input-format.md");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// Every ```` ```json ```` fenced block in `text`, in order of appearance.
fn json_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.trim_end() != "```json" {
            continue;
        }
        let mut block = String::new();
        loop {
            let line = lines
                .next()
                .expect("a ```json fence in docs/input-format.md is never closed");
            if line.trim_end() == "```" {
                break;
            }
            block.push_str(line);
            block.push('\n');
        }
        blocks.push(block);
    }
    blocks
}

/// The count both tests below assert against, so that a dropped fence or a
/// renamed heading fails loudly here instead of letting either test pass
/// vacuously over zero blocks.
const EXPECTED_BLOCKS: usize = 2;

#[test]
fn the_document_still_contains_its_examples() {
    let blocks = json_blocks(&document());
    assert_eq!(
        blocks.len(),
        EXPECTED_BLOCKS,
        "docs/input-format.md must contain exactly {EXPECTED_BLOCKS} ```json example \
         documents (found {}); update EXPECTED_BLOCKS deliberately when adding one, and \
         keep field fragments out of ```json fences",
        blocks.len()
    );
    for (index, block) in blocks.iter().enumerate() {
        assert!(
            !block.trim().is_empty(),
            "example {index} in docs/input-format.md is empty"
        );
    }
}

#[test]
fn every_documented_example_parses_as_a_problem() {
    let blocks = json_blocks(&document());
    assert_eq!(blocks.len(), EXPECTED_BLOCKS, "see the companion test");

    for (index, block) in blocks.iter().enumerate() {
        let problem: Problem = serde_json::from_str(block).unwrap_or_else(|error| {
            panic!("example {index} in docs/input-format.md does not parse: {error}\n{block}")
        });

        // A document that parses but cannot be solved would still be a
        // broken example, so drive the whole pipeline over it.
        let result = run(&problem, None).unwrap_or_else(|error| {
            panic!("example {index} in docs/input-format.md does not solve: {error}")
        });
        assert_eq!(
            result.frequencies_hz, problem.frequencies_hz,
            "example {index} swept the wrong frequencies"
        );
        assert_eq!(
            result.impedance_ohm.len(),
            problem.frequencies_hz.len(),
            "example {index} returned the wrong number of impedance matrices"
        );
        for matrix in &result.impedance_ohm {
            assert_eq!(
                matrix.nrows(),
                problem.ports.len(),
                "example {index} returned a matrix that does not match its port count"
            );
        }
    }
}

/// The document states that the first example is the minimal one: the
/// smallest document that still solves. Pin the two claims that makes — one
/// segment, one port — so the prose and the example cannot disagree.
#[test]
fn the_first_example_is_the_minimal_one() {
    let blocks = json_blocks(&document());
    let minimal: Problem = serde_json::from_str(&blocks[0]).expect("the minimal example parses");
    assert_eq!(minimal.geometry.segment_count(), 1);
    assert_eq!(minimal.ports.len(), 1);
    assert!(
        minimal.coupling.is_trivial(),
        "the minimal example sets no coupling"
    );
}

/// The document's claim about the strictness boundary: unknown keys are
/// rejected at the top level of the document, but *not* inside a
/// `discretization` payload. Both halves are load-bearing prose, so pin both.
#[test]
fn the_documented_strictness_boundary_holds() {
    let minimal = json_blocks(&document()).swap_remove(0);

    let top_level = minimal.replace("\"ports\":", "\"typo\": true, \"ports\":");
    let error = serde_json::from_str::<Problem>(&top_level)
        .expect_err("an unknown key at the top level is rejected");
    assert!(error.to_string().contains("typo"), "{error}");

    let in_payload = minimal.replace("\"nw\": 3,", "\"typo\": 1, \"nw\": 3,");
    assert_ne!(in_payload, minimal, "the replacement must actually apply");
    serde_json::from_str::<Problem>(&in_payload)
        .expect("an unknown key inside a discretization payload is accepted, as documented");
}

/// The document's discretization table gives, for each of the five schemes,
/// the JSON key and an example payload in inline code. Rebuild the tagged
/// object from each row and parse it, so the table's spellings are checked
/// against the enum rather than merely transcribed from its serde
/// attributes.
#[test]
fn the_documented_discretization_spellings_all_parse() {
    use fasterhenry::solve::Discretization;

    let text = document();
    let table = text
        .split("## `discretization`")
        .nth(1)
        .expect("the document still has a `discretization` section");

    let mut parsed = Vec::new();
    for line in table.lines() {
        // Only the scheme table's rows: `| `key` | `{payload}` | prose |`.
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 4 {
            continue;
        }
        let (Some(key), Some(payload)) = (
            cells[1].strip_prefix('`').and_then(|c| c.strip_suffix('`')),
            cells[2].strip_prefix('`').and_then(|c| c.strip_suffix('`')),
        ) else {
            continue;
        };
        if !payload.starts_with('{') && !payload.starts_with('[') {
            continue;
        }
        let tagged = format!("{{ {key:?}: {payload} }}");
        let discretization: Discretization =
            serde_json::from_str(&tagged).unwrap_or_else(|error| {
                panic!("documented `{key}` payload does not parse: {error}\n{tagged}")
            });
        parsed.push((key.to_owned(), discretization));
    }

    let keys: Vec<&str> = parsed.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "uniform",
            "per_segment",
            "graded",
            "skin_depth",
            "per_segment_graded"
        ],
        "docs/input-format.md must document every discretization scheme, in enum order"
    );

    // Each row produced the variant its key names, so no two rows document
    // the same scheme under different spellings.
    for (key, discretization) in &parsed {
        let variant = match discretization {
            Discretization::Uniform(_) => "uniform",
            Discretization::PerSegment(_) => "per_segment",
            Discretization::Graded(_) => "graded",
            Discretization::SkinDepth(_) => "skin_depth",
            Discretization::PerSegmentGraded(_) => "per_segment_graded",
            _ => panic!("a new discretization scheme exists and is undocumented"),
        };
        assert_eq!(key, variant, "documented key `{key}` parsed as `{variant}`");
    }
}
