//! Deterministic counter-based pseudorandom number generator for the
//! POWDERBURN simulation kernel.
//!
//! Uses `splitmix64` as the underlying hash primitive.  Every `draw` call
//! is a pure function of its five named inputs — no mutable state, no global
//! generator.  This guarantees perfect replay: the same (seed, scenario_id,
//! tick, actor_id, stream) tuple always produces the same result.
//!
//! Rejection sampling (never modulo) avoids statistical bias in the output
//! range [lo, hi].

#![forbid(unsafe_code)]

use core::fmt;

/// Tag identifying which gameplay stream is drawing a random number.
///
/// Each stream uses an independent hash domain so that biases in one stream
/// (e.g. always checking ToHit first) cannot systematically affect another
/// stream's outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamTag {
    ToHit,
    Damage,
    Crit,
    Misfire,
    Scatter,
    Morale,
    AiTieBreak,
    Loot,
}

impl StreamTag {
    /// Return a unique u64 discriminant for this stream tag.
    const fn discriminant(self) -> u64 {
        match self {
            StreamTag::ToHit => 0,
            StreamTag::Damage => 1,
            StreamTag::Crit => 2,
            StreamTag::Misfire => 3,
            StreamTag::Scatter => 4,
            StreamTag::Morale => 5,
            StreamTag::AiTieBreak => 6,
            StreamTag::Loot => 7,
        }
    }
}

impl fmt::Display for StreamTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamTag::ToHit => write!(f, "ToHit"),
            StreamTag::Damage => write!(f, "Damage"),
            StreamTag::Crit => write!(f, "Crit"),
            StreamTag::Misfire => write!(f, "Misfire"),
            StreamTag::Scatter => write!(f, "Scatter"),
            StreamTag::Morale => write!(f, "Morale"),
            StreamTag::AiTieBreak => write!(f, "AiTieBreak"),
            StreamTag::Loot => write!(f, "Loot"),
        }
    }
}

/// SplitMix64: a fast, high-quality 64-bit pseudorandom number generator.
///
/// Given a 64-bit seed, produces a sequence of 64-bit values.  We use it
/// not as a sequential generator but as a *keyed hash* of the five draw
/// arguments: we mix them into the seed and run one splitmix64 round to
/// produce a deterministic output.
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15u64);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9u64);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111ebu64);
    z ^ (z >> 31)
}

/// A counter-based deterministic RNG.
///
/// `PbRng` has no mutable state.  All randomness is derived by hashing the
/// five inputs to `draw` through a deterministic function.
#[derive(Debug)]
pub struct PbRng;

impl PbRng {
    /// Draw a deterministic integer in `[lo, hi]` (inclusive).
    ///
    /// Uses rejection sampling to avoid modulo bias.  The range must satisfy
    /// `lo <= hi`; if `lo == hi` the result is trivially `lo`.
    ///
    /// # Panics
    ///
    /// Panics if `lo > hi`.
    pub fn draw(
        seed: u64,
        scenario_id: u32,
        tick: u64,
        actor_id: u32,
        stream: StreamTag,
        lo: i32,
        hi: i32,
    ) -> i32 {
        assert!(lo <= hi, "PbRng::draw: lo ({}) > hi ({})", lo, hi);

        let range = (hi as u64).wrapping_sub(lo as u64).wrapping_add(1);
        if range == 0 {
            // Full u64 range — no rejection needed.
            // But this can't happen for i32 range since hi-lo+1 fits in u64.
            return lo;
        }
        if range == 1 {
            return lo;
        }

        // Mix the five inputs into a single 64-bit seed for splitmix64.
        let mix_key: u64 = seed
            .wrapping_mul(0x9e3779b97f4a7c15u64)
            .wrapping_add(scenario_id as u64)
            .wrapping_mul(0xbf58476d1ce4e5b9u64)
            .wrapping_add(tick)
            .wrapping_mul(0x94d049bb133111ebu64)
            .wrapping_add(actor_id as u64)
            .wrapping_mul(0x9e3779b97f4a7c15u64)
            .wrapping_add(stream.discriminant());

        // Rejection sampling: compute threshold = floor(U64_MAX / range) * range
        // If the hash is >= threshold, try again (but since we re-mix, the next
        // hash is guaranteed different).
        let threshold = u64::MAX - (u64::MAX % range);

        let mut attempt = mix_key;
        loop {
            let hash = splitmix64(attempt);
            if hash < threshold {
                let offset = hash % range;
                // Safe: lo + offset fits in i32 because offset < range <= hi-lo+1.
                return (lo as u64).wrapping_add(offset) as i32;
            }
            // Mix attempt for next iteration.
            attempt = attempt.wrapping_add(0x9e3779b97f4a7c15u64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_deterministic_same_inputs_same_output() {
        let a = PbRng::draw(42, 1, 100, 5, StreamTag::ToHit, 0, 100);
        let b = PbRng::draw(42, 1, 100, 5, StreamTag::ToHit, 0, 100);
        assert_eq!(a, b);
    }

    #[test]
    fn draw_different_seeds_different_output() {
        let a = PbRng::draw(42, 1, 100, 5, StreamTag::ToHit, 0, 100);
        let b = PbRng::draw(99, 1, 100, 5, StreamTag::ToHit, 0, 100);
        // Extremely unlikely to collide.
        assert_ne!(a, b);
    }

    #[test]
    fn draw_different_actor_different_output() {
        let a = PbRng::draw(42, 1, 100, 5, StreamTag::ToHit, 0, 100);
        let b = PbRng::draw(42, 1, 100, 7, StreamTag::ToHit, 0, 100);
        assert_ne!(a, b);
    }

    #[test]
    fn draw_different_stream_different_output() {
        let a = PbRng::draw(42, 1, 100, 5, StreamTag::ToHit, 0, 100);
        let b = PbRng::draw(42, 1, 100, 5, StreamTag::Damage, 0, 100);
        assert_ne!(a, b);
    }

    #[test]
    fn draw_output_in_range() {
        for _ in 0..1000 {
            let val = PbRng::draw(12345, 2, 500, 10, StreamTag::Crit, -50, 50);
            assert!(
                val >= -50 && val <= 50,
                "val {} out of range [-50, 50]",
                val
            );
        }
    }

    #[test]
    fn draw_single_value_range() {
        let val = PbRng::draw(42, 1, 100, 5, StreamTag::ToHit, 7, 7);
        assert_eq!(val, 7);
    }

    #[test]
    fn draw_zero_range() {
        let val = PbRng::draw(42, 1, 100, 5, StreamTag::ToHit, 0, 0);
        assert_eq!(val, 0);
    }

    #[test]
    fn draw_negative_range() {
        for _ in 0..1000 {
            let val = PbRng::draw(999, 3, 200, 8, StreamTag::Misfire, -100, -1);
            assert!(
                val >= -100 && val <= -1,
                "val {} out of range [-100, -1]",
                val
            );
        }
    }

    #[test]
    fn draw_distribution_sanity() {
        // Check that over many draws we see a reasonable spread of values.
        // This is a statistical sanity check, not a rigorous test.
        let mut sum: i64 = 0;
        let count = 10_000;
        for i in 0..count {
            let val = PbRng::draw(42, 1, i as u64, 5, StreamTag::Damage, 0, 100);
            sum += val as i64;
        }
        // For uniform [0,100], expected mean is 50. Accept a wide tolerance.
        let mean_times_1000 = (sum * 1000) / count as i64;
        // Expected mean * 1000 = 50000, accept 10000 <= mean_times_1000 <= 90000
        assert!(
            mean_times_1000 >= 10000 && mean_times_1000 <= 90000,
            "mean_times_1000 {} too far from expected 50000",
            mean_times_1000
        );
    }

    #[test]
    fn stream_tag_display() {
        assert_eq!(StreamTag::ToHit.to_string(), "ToHit");
        assert_eq!(StreamTag::Damage.to_string(), "Damage");
        assert_eq!(StreamTag::Crit.to_string(), "Crit");
        assert_eq!(StreamTag::Misfire.to_string(), "Misfire");
        assert_eq!(StreamTag::Scatter.to_string(), "Scatter");
        assert_eq!(StreamTag::Morale.to_string(), "Morale");
        assert_eq!(StreamTag::AiTieBreak.to_string(), "AiTieBreak");
        assert_eq!(StreamTag::Loot.to_string(), "Loot");
    }
}
