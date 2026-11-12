//! `own-flip-faith-war-asks-no-power`: the small-world faith war opened on
//! our own flip, the founder alone on its faith and half our cities leaning
//! to it, opens at any power.
//!
//! **Why (live Deity Online, 4-player Tiny Pangaea, Gran Colombia, no faith
//! founded; 62 games from 2026-11-11 19:57Z to 11-12 11:00Z).** Ten ended
//! on a rival Religious Victory. In eight of them our own majority flipped
//! before the faith held another major while we still held out: those
//! losses ran through the small-world own-flip reading (`faith_war_small_world`: only the founder
//! holds its faith, at least [`SMALL_WORLD_OWN_CITIES`] cities of ours, at
//! least [`SMALL_WORLD_OWN_LEANING`] and [`SMALL_WORLD_OWN_SHARE`]% of them
//! following or converting to it), 3 to 39 turns before our flip. Of the 43
//! games that entered that reading and were decided, 1 of the 14 that
//! opened a faith war on the founder before our flip lost on Religion,
//! against 7 of the 29 that did not; our power did not separate them
//! (median 0.40 times the founder's steady power in the losses, 0.42 in the
//! rest). The war was held on power: four of the 2026-11-12 losses read our
//! power at 0.21-0.37 times the founder's steady power when it was held
//! (civvis-20261112T091845Z Canada 0.21, 094511Z Russia 0.35, 103652Z Cree
//! 0.37, 104039Z Egypt 0.25), under the reading's own floor
//! [`SMALL_WORLD_OWN_POWER`] and version 2's 0.7. The wars the seat did
//! open under 0.7 times the founder's steady power (18 games) lost 0.22
//! cities on average in the next 30 turns, all 18 reached turn 100 and none
//! of the 17 decided ended on Religion; the 37 held under 0.7 times lost
//! 0.38 cities, 30 of 35 reached turn 100 and 6 of 30 ended on Religion.
//!
//! **What the gene does.** Riding `faith-war-on-a-small-world`, the own-flip
//! reading no longer asks [`SMALL_WORLD_OWN_POWER`]
//! (`faith_war_small_world_race`); its city thresholds are unchanged. When
//! that reading alone opens the war (`own_flip_asks_no_power`: the shipped
//! reading, with the floor, would not), the faith war opens without version
//! 2's power floor, `faith-war-needs-the-edge` or the rout ratio, and keeps
//! every other hold: the threatened-city and falling-city holds, the
//! major-war limit, the treasury reserve, the income gate, legality and
//! version 2's cooldown. When the host allows no declaration on the founder
//! yet, the seat denounces it so a Formal War follows (`faith_war_denounce`).
//! The opening, the holds and the keep are journalled under the gene's tag
//! with the power ratio it waived. The declaration desk's counter-war floor
//! (`counter_floor_holds`) does not hold a founder on that reading.
//!
//! Off: unchanged.

use super::faith_war_before_the_flip::{
    SMALL_WORLD_OWN_CITIES, SMALL_WORLD_OWN_LEANING, SMALL_WORLD_OWN_POWER, SMALL_WORLD_OWN_SHARE,
};
use super::*;

impl AdvancedAi {
    /// `own-flip-faith-war-asks-no-power`: whether `rival`'s small-world
    /// reading holds on our own flip whatever our power (the founder's faith
    /// at the own-flip thresholds). `false` with the gene off.
    pub(crate) fn own_flip_reading(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.own_flip_faith_war_asks_no_power
            && self
                .faith_war_small_world_race(g, pid, rival, true)
                .is_some_and(|(_, own_flip)| own_flip)
    }

    /// `own-flip-faith-war-asks-no-power`: our power as a share of `rival`'s
    /// steady power when the gene alone opens the war on it: the own-flip
    /// reading holds and the shipped reading, which asks
    /// [`SMALL_WORLD_OWN_POWER`], does not. `None` otherwise, and always
    /// with the gene off.
    pub(crate) fn own_flip_asks_no_power(&self, g: &Game, pid: usize, rival: usize) -> Option<f64> {
        if !self.own_flip_reading(g, pid, rival)
            || self
                .faith_war_small_world_race(g, pid, rival, false)
                .is_some()
        {
            return None;
        }
        Some(g.military_power(pid) / self.steady_rival_power(g, rival).max(1.0))
    }

    /// The journal's account of the waived power: `ours` against their
    /// steady power, as the ratio and the floors it is under.
    pub(crate) fn own_flip_power_waived(&self, g: &Game, pid: usize, rival: usize) -> String {
        let ours = g.military_power(pid);
        let steady = self.steady_rival_power(g, rival).max(1.0);
        format!(
            "ours {ours:.0} against their steady {steady:.0} ({:.2}x), under the {SMALL_WORLD_OWN_POWER:.1}x own-flip floor",
            ours / steady
        )
    }

    /// The reading the journal names: the founder alone on its faith and
    /// how many of our cities lean to it.
    pub(crate) fn own_flip_cities(g: &Game, pid: usize, rival: usize) -> String {
        let cities = g.player_city_ids(pid).len();
        let leaning = Self::faith_war_faith(g, rival)
            .map_or(0, |faith| Self::our_cities_on_the_faith(g, pid, faith));
        format!(
            "only its founder follows its faith and {leaning} of our {cities} cities lean to it (at least {SMALL_WORLD_OWN_LEANING} of {SMALL_WORLD_OWN_CITIES} and {SMALL_WORLD_OWN_SHARE}%), so our flip is next"
        )
    }
}

#[cfg(test)]
mod tests;
