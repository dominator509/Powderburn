//! Seven-attribute character model and deterministic derived statistics.

#![forbid(unsafe_code)]

/// The seven core character attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Attributes {
    pub grit: i32,
    pub nerve: i32,
    pub wind: i32,
    pub hands: i32,
    pub eyes: i32,
    pub savvy: i32,
    pub luck: i32,
}

impl Attributes {
    pub const MIN: i32 = 1;
    pub const MAX: i32 = 10;
    pub const CREATION_MAX: i32 = 9;
    pub const CREATION_TOTAL: i32 = 40;

    /// Balanced defaults used by proving scenarios that predate authored
    /// attribute loading. They obey every character-creation invariant.
    pub const BALANCED: Self = Self {
        grit: 6,
        nerve: 6,
        wind: 6,
        hands: 6,
        eyes: 6,
        savvy: 5,
        luck: 5,
    };

    pub const fn total(self) -> i32 {
        self.grit + self.nerve + self.wind + self.hands + self.eyes + self.savvy + self.luck
    }

    pub fn validate(self) -> Result<(), &'static str> {
        if self
            .values()
            .iter()
            .any(|value| !(Self::MIN..=Self::MAX).contains(value))
        {
            return Err("attributes must be in the range 1..=10");
        }
        Ok(())
    }

    pub fn validate_creation(self) -> Result<(), &'static str> {
        self.validate()?;
        if self
            .values()
            .iter()
            .any(|value| *value > Self::CREATION_MAX)
        {
            return Err("no creation attribute may exceed 9");
        }
        if self.total() != Self::CREATION_TOTAL {
            return Err("creation attributes must total 40");
        }
        Ok(())
    }

    pub const fn action_points(self) -> i32 {
        5 + self.wind / 2
    }

    pub const fn hit_points(self, level: u32) -> i32 {
        20 + self.grit * 3 + level as i32 * 2
    }

    pub const fn sand(self) -> i32 {
        10 + self.nerve * 2
    }

    pub const fn evasion(self, stance_modifier: i32, cover_modifier: i32) -> i32 {
        5 + self.hands / 2 + stance_modifier + cover_modifier
    }

    pub const fn sight_radius(self) -> i32 {
        8 + self.eyes
    }

    pub const fn carry_load(self) -> i32 {
        25 + self.grit * 5
    }

    pub const fn sequence(self) -> i32 {
        2 + (self.eyes + self.hands) / 4
    }

    const fn values(self) -> [i32; 7] {
        [
            self.grit, self.nerve, self.wind, self.hands, self.eyes, self.savvy, self.luck,
        ]
    }
}

impl Default for Attributes {
    fn default() -> Self {
        Self::BALANCED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_attributes_and_derived_stats_match_spec() {
        let attributes = Attributes::BALANCED;
        assert_eq!(attributes.validate_creation(), Ok(()));
        assert_eq!(attributes.action_points(), 8);
        assert_eq!(attributes.hit_points(1), 40);
        assert_eq!(attributes.sand(), 22);
        assert_eq!(attributes.evasion(2, 15), 25);
        assert_eq!(attributes.sight_radius(), 14);
        assert_eq!(attributes.carry_load(), 55);
        assert_eq!(attributes.sequence(), 5);
    }
}
