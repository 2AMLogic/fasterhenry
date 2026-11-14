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
/// rejected at the top level, in `geometry`, in each node and segment, and in
/// `coupling` — but *not* inside a `discretization` payload or a port object.
///
/// Every member of that boundary is load-bearing prose, so pin every member,
/// not a sample of it. An earlier revision of this test pinned only the
/// top-level rejection and the payload acceptance, and the one unpinned claim
/// was the wrong one: the document had `ports` on the strict side, while
/// `Port` carries no `deny_unknown_fields`. A table-driven sweep means the
/// next edit to that sentence cannot drift the same way.
#[test]
fn the_documented_strictness_boundary_holds() {
    let minimal = json_blocks(&document()).swap_remove(0);

    // `insertion` is spliced in immediately before the *unique* occurrence of
    // `anchor` in the minimal example, placing an unknown key at `level`.
    // `rejected` is what docs/input-format.md promises happens then.
    let cases = [
        ("the top level", "\"ports\":", "\"typo\": true, ", true),
        ("geometry", "\"nodes\":", "\"typo\": true, ", true),
        ("a node", "\"x\": 0.0,", "\"typo\": true, ", true),
        ("a segment", "\"a\": 0,", "\"typo\": true, ", true),
        ("a port", "\"positive\": 0,", "\"typo\": true, ", false),
        (
            "coupling",
            "\"frequencies_hz\":",
            "\"coupling\": { \"typo\": true }, ",
            true,
        ),
        (
            "a discretization payload",
            "\"nw\": 3,",
            "\"typo\": 1, ",
            false,
        ),
    ];

    for (level, anchor, insertion, rejected) in cases {
        assert_eq!(
            minimal.matches(anchor).count(),
            1,
            "the minimal example must contain `{anchor}` exactly once, so the \
             insertion for {level} lands where this case intends"
        );
        let mutated = minimal.replace(anchor, &format!("{insertion}{anchor}"));

        match (rejected, serde_json::from_str::<Problem>(&mutated)) {
            (true, Err(error)) => assert!(
                error.to_string().contains("typo"),
                "an unknown key in {level} is rejected, but not by name: {error}"
            ),
            (true, Ok(_)) => panic!(
                "docs/input-format.md says an unknown key in {level} is rejected, \
                 but this parsed:\n{mutated}"
            ),
            (false, Ok(_)) => {}
            (false, Err(error)) => panic!(
                "docs/input-format.md says an unknown key in {level} is accepted \
                 and ignored, but it was rejected: {error}"
            ),
        }
    }
}

/// The `ports` section spells out what the missing `deny_unknown_fields`
/// costs a reader: a misspelled optional `name` is dropped in silence, while a
/// misspelled *required* key is still caught — but reported as the key that is
/// now missing, not as the typo that was written. Both are concrete claims
/// about observable behaviour, so pin both.
#[test]
fn the_documented_port_typo_consequences_hold() {
    let minimal = json_blocks(&document()).swap_remove(0);

    let misspelled_name =
        minimal.replace("\"positive\": 0,", "\"nmae\": \"sense\", \"positive\": 0,");
    let problem: Problem = serde_json::from_str(&misspelled_name)
        .expect("a misspelled optional port key is accepted, as documented");
    assert_eq!(
        problem.ports[0].name, None,
        "a misspelled `name` must leave the port unnamed, as documented"
    );

    let misspelled_required = minimal.replace("\"positive\": 0,", "\"positiv\": 0,");
    assert_ne!(
        misspelled_required, minimal,
        "the replacement must actually apply"
    );
    let error = serde_json::from_str::<Problem>(&misspelled_required)
        .expect_err("a misspelled required port key is still caught");
    assert!(
        error.to_string().contains("missing field `positive`"),
        "a misspelled required port key is reported as the missing key, not the \
         typo written: {error}"
    );
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
