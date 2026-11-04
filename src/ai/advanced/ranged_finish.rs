//! `ranged-finish-on-the-mean`: the live finishing volley commits a ranged-only
//! line on its expected roll, not its lowest.
//!
//! `finishing::finish_live_war_units_excluding` proves a kill on a private
//! board, rejects a line that would leave an attacker dead, and then asks the
//! line's damage at the lowest 0.8 roll (`live_finishing_damage_floor`) to
//! cover the target's health, adding a reserve attacker where one exists. Over
//! 60 live Immortal runs (2026-11-03/04) the pass made 766 volleys and refused
//! 811: 539 on survival, which this gene leaves alone, and 272 on the roll.
//! Meanwhile 694 enemy units were left alive at 30 health or less after our
//! attacks, 261 of them with a healthy unit of ours idle within strike reach.
//!
//! A ranged blow draws no return damage, so a ranged line the floor refuses
//! costs only the shot when it falls short, and leaves the target lower for
//! the next shooter. Under the gene, a line whose every attacker (the reserve
//! included) is ranged commits when its centre-roll damage covers the
//! target's health. Melee lines keep the floor. `Kind::OptIn`; with the gene
//! off the floor is the only test.
use super::AdvancedAi;

impl AdvancedAi {
    /// Whether the live finishing volley may commit a ranged-only line on its
    /// centre roll. See `advanced/ranged_finish.rs`.
    pub fn ranged_finishes_on_the_mean(&self) -> bool {
        self.ranged_finish_on_the_mean
    }
}
