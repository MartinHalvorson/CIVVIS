//! `obsolete-army-spends-the-floor` (opt-in): while the empire is at peace
//! and at least [`OBSOLETE_ARMY_SHARE`] of its military (and at least
//! [`OBSOLETE_ARMY_MIN_UNITS`] units) stands [`OBSOLETE_ERA_GAP`] or more
//! eras behind the world, the peacetime upgrade pass spends down to the
//! wartime floor ([`OBSOLETE_ARMY_FLOOR`]) instead of the peacetime 120. The
//! upkeep reserve (`upgrade_treasury_floor`) still applies on top.
//!
//! **Why (live Deity Online Korea, 2026-11-05).**
//! - At turn 145, across 19 Deity runs, a median 8 of our 17 military units
//!   were Ancient-era (Warrior, Archer, Slinger, Spearman, Chariot): 56% of
//!   all military units. The treasury held a median 246 Gold (about +40 a
//!   turn).
//! - With the retire rules off (operator, 11-05 ~03:1xZ), late wars decide
//!   games: 3 of 11 games after 02:01Z ended in our elimination. 033244Z lost
//!   29 healthy units t150-185, 21 of them to Machine Guns, the victims
//!   mostly Warriors (10) and Archers (7), though the upgrade techs were known.
//! - The upgrade pass works when it can: across 119 snapshots (t100-150) an
//!   affordable upgrade inside our borders existed on 22% of turns, and the
//!   live game took about that many. Each upgrade costs ~195-205 Gold
//!   (Archer → Hwacha, Warrior → Musketman), so with the 120 floor a turn
//!   needs ~320 Gold, against the median ~250. 050752Z banked 412-630 Gold
//!   t120-150 and took one upgrade every ~5 turns.
//!
//! Upgrading is cheap modernization: an Archer → Hwacha upgrade is ~195 Gold
//! against roughly three times that to buy a Hwacha. Off, nothing changes.

use super::*;

/// The share of military units this many eras behind that makes the army
/// obsolete.
pub(super) const OBSOLETE_ARMY_SHARE: f64 = 0.5;

/// Fewer military units than this are never an obsolete army.
pub(super) const OBSOLETE_ARMY_MIN_UNITS: usize = 3;

/// Eras behind the world era that make a unit obsolete.
pub(super) const OBSOLETE_ERA_GAP: usize = 2;

/// The treasury floor an obsolete army's upgrade pass keeps at peace: the
/// wartime floor of `BasicAi::upgrade_units`.
pub(super) const OBSOLETE_ARMY_FLOOR: f64 = 30.0;

impl AdvancedAi {
    /// Whether `pid`'s military is mostly [`OBSOLETE_ERA_GAP`]+ eras behind
    /// the world era. Land, sea and air military units count; support and
    /// civilian units do not.
    pub(super) fn army_is_obsolete(g: &Game, pid: usize) -> bool {
        let world = g.world_era;
        let mut military = 0usize;
        let mut obsolete = 0usize;
        for uid in g.player_unit_ids(pid) {
            let kind = g.units[&uid].kind;
            let Some(spec) = g.rules.units.get(&kind) else {
                continue;
            };
            if spec.class != "military" {
                continue;
            }
            military += 1;
            if g.unit_era(kind.as_str()) + OBSOLETE_ERA_GAP <= world {
                obsolete += 1;
            }
        }
        military >= OBSOLETE_ARMY_MIN_UNITS
            && obsolete as f64 >= military as f64 * OBSOLETE_ARMY_SHARE
    }

    /// The peacetime upgrade floor under the gene: the wartime floor while
    /// the army is obsolete, `peace` otherwise. Always `peace` with the gene
    /// off.
    pub(super) fn obsolete_army_upgrade_floor(&self, g: &Game, pid: usize, peace: f64) -> f64 {
        if self.obsolete_army_spends_the_floor && Self::army_is_obsolete(g, pid) {
            OBSOLETE_ARMY_FLOOR.min(peace)
        } else {
            peace
        }
    }
}

#[cfg(test)]
mod tests;
