//! A per-thread memo of aligned-bar evaluations, keyed by the geometry.
//!
//! The Neumann integral of two parallel bars with aligned cross-section axes
//! depends on nothing but their six extents and the offset between their
//! centres: translate the pair anywhere in space and the answer is the same.
//! A bundle of `nw × nh` filaments cut from one trace therefore contains only
//! `(2nw − 1)(2nh − 1)` distinct relative positions, of which the canonical
//! pair ordering of [`super::canonical`] keeps half — 53 for the 8 × 4 bundle
//! of `benches/kernels.rs`, against its 528 unordered pairs. Every evaluation
//! beyond the first of each repeats work already done, and the repeats are
//! the expensive kind: the whole bundle takes
//! [`Method::AlignedQuadrature`](super::Method::AlignedQuadrature), tens of
//! microseconds a pair against a fraction of one for a far-field point
//! quadrature.
//!
//! The memo is a fixed-size direct-mapped table, one per thread: a lookup is
//! one hash and one key comparison, and a miss overwrites the slot it lands
//! in. That bounds both the memory (the table never grows) and the cost on a
//! geometry whose pairs never repeat, where the memo degenerates to a hash
//! per pair against a kernel a thousand times dearer.
//!
//! Memoization cannot move a value: a hit returns the very bits the first
//! evaluation of a bit-identical problem produced, so the accuracy and the
//! exact symmetry of an assembly are the same with the table as without it.

use std::cell::RefCell;

use super::aligned::AlignedBars;
use super::Method;

/// Slots per thread — 96 KiB, enough to hold every distinct pair of a bundle
/// of a few hundred filaments at once.
const SLOTS: usize = 1024;

/// The bit patterns of an [`AlignedBars`]: its nine numbers are the entire
/// input to an evaluation, so equal bits mean an identical problem and
/// therefore a bit-identical answer.
type Key = [u64; 9];

#[derive(Clone, Copy)]
struct Entry {
    /// Whether this slot has ever been written. Cheaper to carry than to
    /// reserve a key pattern that no geometry can produce.
    occupied: bool,
    key: Key,
    value: f64,
    method: Method,
}

thread_local! {
    static TABLE: RefCell<Box<[Entry]>> = RefCell::new(
        vec![
            Entry {
                occupied: false,
                key: [0; 9],
                value: 0.0,
                method: Method::BarClosedForm,
            };
            SLOTS
        ]
        .into_boxed_slice(),
    );
}

fn key_of(bars: &AlignedBars) -> Key {
    [
        bars.length[0].to_bits(),
        bars.length[1].to_bits(),
        bars.width[0].to_bits(),
        bars.width[1].to_bits(),
        bars.height[0].to_bits(),
        bars.height[1].to_bits(),
        bars.offset[0].to_bits(),
        bars.offset[1].to_bits(),
        bars.offset[2].to_bits(),
    ]
}

/// Multiplicative hash of the whole key. Only the slot depends on it — a hit
/// is confirmed by comparing the key itself — so a poor mix costs collisions,
/// never a wrong answer.
fn slot(key: &Key) -> usize {
    const MIX: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut hash = MIX;
    for &word in key {
        hash = (hash ^ word).wrapping_mul(MIX).rotate_left(29);
    }
    // A bundle's keys share their first seven words bit-for-bit (same
    // extents) and differ only in the width/height offset — so the folding
    // loop above enters its last two rounds from an identical state every
    // time, and only those two words' low-entropy bit patterns (small
    // integer multiples of a cell size) drive the outcome. Reading a fixed
    // ten-bit window straight out of that under-mixed state clusters real
    // bundles into a handful of slots; a SplitMix64-style avalanche spreads
    // whatever entropy is there across every bit first.
    hash ^= hash >> 30;
    hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    hash ^= hash >> 27;
    hash = hash.wrapping_mul(0x94d0_49bb_1331_11eb);
    hash ^= hash >> 31;
    hash as usize & (SLOTS - 1)
}

/// The aligned-bar evaluation of `bars`, from this thread's table when it is
/// there and from `evaluate` (recorded for next time) when it is not.
pub(super) fn aligned_integral(
    bars: &AlignedBars,
    evaluate: impl FnOnce(&AlignedBars) -> (f64, Method),
) -> (f64, Method) {
    let key = key_of(bars);
    let index = slot(&key);
    let hit = TABLE.with_borrow(|table| {
        let entry = &table[index];
        (entry.occupied && entry.key == key).then_some((entry.value, entry.method))
    });
    if let Some(hit) = hit {
        return hit;
    }
    // Deliberately outside the borrow: the kernel is long-running, and
    // nothing it calls may find the table already borrowed.
    let (value, method) = evaluate(bars);
    TABLE.with_borrow_mut(|table| {
        table[index] = Entry {
            occupied: true,
            key,
            value,
            method,
        };
    });
    (value, method)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A family of distinct keys, indexed so that the expected value is a
    /// function of the index alone.
    ///
    /// The extents are negative: the memo never interprets the geometry, only
    /// its bits, and a table shared by every test on a thread must not be
    /// seeded with made-up values under a key a real evaluation could ask for.
    fn bars(index: usize) -> AlignedBars {
        AlignedBars {
            length: [-1.0, -1.0 - index as f64],
            width: [-0.2, -0.3],
            height: [-0.1, -0.1],
            offset: [index as f64 * 1e-3, 0.5, 0.25],
        }
    }

    #[test]
    fn a_hit_returns_the_recorded_bits_and_method() {
        let b = bars(7);
        let first = aligned_integral(&b, |_| (std::f64::consts::PI, Method::AlignedQuadrature));
        // A second call must not reach the evaluator at all.
        let second = aligned_integral(&b, |_| panic!("evaluated twice"));
        assert_eq!(first, (std::f64::consts::PI, Method::AlignedQuadrature));
        assert_eq!(second, first);
    }

    /// A bundle's pairs share six of the key's nine words bit-for-bit (every
    /// filament has the same extents) and its seventh (axial offset) is
    /// exactly zero for every cross-section-only pair, so only the last two
    /// words — the width and height offset — carry any entropy at all. An
    /// under-mixed hash can fold seven identical words into an identical
    /// running state and then read a fixed bit window straight out of the
    /// last two, which clusters every real bundle into a handful of slots
    /// regardless of table size (measured on the 8×4 fixture of
    /// `benches/kernels.rs` before this test existed: 53 distinct keys, but
    /// misses landing in only 23 of the 1024 slots, one of them 631 times —
    /// a >30x reload rate on geometry the table had already seen). `slot`
    /// must spread keys that differ only in their last two words across
    /// most of the table, not a handful of buckets.
    #[test]
    fn keys_differing_only_in_the_last_two_words_spread_across_the_table() {
        use std::collections::HashSet;
        let mut slots = HashSet::new();
        // 15 width offsets x 7 height offsets = 105, matching the (2*8-1) x
        // (2*4-1) relative positions of the bundle fixture before the
        // canonical pair ordering halves them to 53.
        for dw in -7..=7 {
            for dh in -3..=3 {
                let bars = AlignedBars {
                    length: [10e-3, 10e-3],
                    width: [1e-3 / 8.0, 1e-3 / 8.0],
                    height: [35e-6 / 4.0, 35e-6 / 4.0],
                    offset: [0.0, dw as f64 * (1e-3 / 8.0), dh as f64 * (35e-6 / 4.0)],
                };
                slots.insert(slot(&key_of(&bars)));
            }
        }
        // A generous, regression-proof floor: the pre-fix hash (fold the
        // nine words, then read bits 40-49 of the raw accumulator) spread
        // the bundle fixture's real 53 keys across only 23 of 1024 slots
        // (one slot answering for 6 different keys in turn, 631 times over
        // a five-batch run); 40-of-105 here would already be a large
        // improvement on that, without demanding textbook-uniform spread.
        assert!(
            slots.len() >= 40,
            "105 bundle-shaped keys used only {} of {SLOTS} slots",
            slots.len()
        );
    }

    /// Far more distinct keys than slots, so collisions are certain: every
    /// one must evict, never answer for the geometry it displaced.
    #[test]
    fn colliding_keys_never_answer_for_each_other() {
        let expected = |index: usize| 1.0 + index as f64;
        for round in 0..4 {
            for index in 0..8 * SLOTS {
                let got = aligned_integral(&bars(index), |b| {
                    assert_eq!(b.offset[0], index as f64 * 1e-3);
                    (expected(index), Method::AlignedQuadrature)
                });
                assert_eq!(got.0, expected(index), "round {round}, index {index}");
            }
        }
    }

    /// Keys differing in one bit of one field must not share an answer.
    #[test]
    fn every_field_is_part_of_the_key() {
        let base = AlignedBars::same(-1.0, -0.2, -0.1);
        let fields: [fn(&mut AlignedBars); 9] = [
            |b| b.length[0] += 1e-9,
            |b| b.length[1] += 1e-9,
            |b| b.width[0] += 1e-9,
            |b| b.width[1] += 1e-9,
            |b| b.height[0] += 1e-9,
            |b| b.height[1] += 1e-9,
            |b| b.offset[0] += 1e-9,
            |b| b.offset[1] += 1e-9,
            |b| b.offset[2] += 1e-9,
        ];
        let seed = aligned_integral(&base, |_| (1.0, Method::AlignedQuadrature));
        assert_eq!(seed.0, 1.0);
        for (which, nudge) in fields.iter().enumerate() {
            let mut moved = base;
            nudge(&mut moved);
            let got = aligned_integral(&moved, |_| (2.0, Method::AlignedQuadrature));
            assert_eq!(got.0, 2.0, "field {which} is missing from the key");
        }
    }
}
