use std::collections::BTreeMap;

use pb_core::event::HitLocationType;
use pb_rng::{PbRng, StreamTag};
use pb_rules::tables;

/// Test hit location distribution over many random draws.
///
/// Verifies that each hit location appears approximately as often as its
/// weight in the table predicts, within a generous tolerance.
#[test]
fn hit_location_distribution() {
    let total_weight = tables::hit_location_total_weight();
    assert_eq!(total_weight, 100, "Hit location weights should sum to 100");

    let seed = 12345u64;
    let scenario = 1u32;
    let mut location_counts: BTreeMap<HitLocationType, u32> = BTreeMap::new();
    let iterations = 100_000;

    for i in 0..iterations {
        let roll = PbRng::draw(
            seed,
            scenario,
            i as u64,
            0,
            StreamTag::Damage,
            0,
            total_weight - 1,
        );
        let loc = tables::select_hit_location(roll).expect("valid location roll");
        *location_counts.entry(loc).or_insert(0) += 1;
    }

    // Check counts are within reasonable bounds
    for entry in tables::HIT_LOCATION_TABLE {
        let count = location_counts.get(&entry.location).copied().unwrap_or(0);
        // Expected count as proportion of total
        let expected = (iterations as f64 * entry.base_chance as f64 / total_weight as f64) as u32;
        let tolerance = (expected as f64 * 0.2) as u32; // 20% tolerance
        let min_expected = expected.saturating_sub(tolerance).max(1);
        let max_expected = expected + tolerance;

        assert!(
            count >= min_expected,
            "Location {:?}: count {} below min {} (expected {})",
            entry.location,
            count,
            min_expected,
            expected
        );
        assert!(
            count <= max_expected,
            "Location {:?}: count {} above max {} (expected {})",
            entry.location,
            count,
            max_expected,
            expected
        );
    }
}

/// Test that select_hit_location covers the full valid range without gaps.
#[test]
fn hit_location_covers_entire_range() {
    let total = tables::hit_location_total_weight();
    let mut seen_indices = vec![false; total as usize];

    for roll in 0..total {
        let loc = tables::select_hit_location(roll).expect("roll in range");
        // Find which index range this location covers
        let mut cumulative = 0i32;
        for entry in tables::HIT_LOCATION_TABLE {
            let start = cumulative;
            cumulative += entry.base_chance;
            if roll >= start && roll < cumulative {
                if entry.location == loc {
                    seen_indices[roll as usize] = true;
                }
            }
        }
    }

    assert!(
        seen_indices.iter().all(|&x| x),
        "Not all rolls map to a location"
    );
}

/// Test called shot accuracy penalties.
#[test]
fn called_shot_penalties_are_consistent() {
    for entry in tables::CALLED_SHOT_TABLE {
        let penalty = tables::called_shot_penalty(entry.location);
        assert_eq!(
            penalty, entry.accuracy_penalty,
            "Penalty mismatch for {:?}: expected {}, got {}",
            entry.location, entry.accuracy_penalty, penalty
        );
    }
}

/// Test that the hit location table entry lookup returns correct entries.
#[test]
fn hit_location_table_entry_consistency() {
    for entry in tables::HIT_LOCATION_TABLE {
        let looked_up = tables::hit_location_entry(entry.location)
            .unwrap_or_else(|| panic!("Entry not found for {:?}", entry.location));
        assert_eq!(looked_up.location, entry.location);
        assert_eq!(looked_up.base_chance, entry.base_chance);
        assert_eq!(looked_up.damage_mult, entry.damage_mult);
        assert_eq!(looked_up.crit_effect, entry.crit_effect);
    }
}
