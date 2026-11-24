//! `raiders-holding-a-settler-are-hunted`: the objective board raises a
//! Destroy row against the hostiles that hold one of our Settlers in the
//! field, and prices it with the Settler.
//!
//! **The finding (Indefinite Benchmarking Tourney, 2026-11-23; 6 of 40
//! reviewed games).** Settlers were built on time and then stood for decades:
//! `civvis-t7-20261123T141222Z` (Lv 5) logged "settler held short" 94 times
//! and founded its t68 city "22 turns since it set out";
//! `civvis-t7-20261123T123605Z` (Lv 8) had 4 cities at t60 and then founded
//! at t73, t78, t85, t86, t88 and t97. The journal is the same in each:
//! "Guard stands with its settler", "Settler keeps (11, 15) through a blocked
//! approach ... the guard holds here until the approach clears", "Settler
//! HELD short ... the safe-step guard rejected every neighbour". The Settler
//! and its guard waited for the raiders to leave, and nothing went to kill
//! them. The military board shows why (141222Z t48-t54, 123605Z t53-t64):
//! - a raider force within six tiles of a city that has a Defend row is
//!   "covered" by that row, so it gets no Destroy row, and the Defend force
//!   only holds the city. That is the Settler waiting beside our own city
//!   at (-9, 30) in 123605Z t53-t73, with a camp at (-8, 28) that was never
//!   cleared;
//! - a barbarian more than six tiles from every city is not on the board at
//!   all, however close it stands to our Settler;
//! - a Destroy row is worth the raiders' own production (a Warrior is 40),
//!   so it ranks below every Escort (a Settler is 280) and Defend row.
//!   Meanwhile the walk it blocks is the empire's next city.
//!
//! **The gene.** For each Settler of ours standing outside a city:
//! - a barbarian within [`SETTLER_HOLD_RADIUS`] of it counts toward the
//!   board's hostile forces, wherever our cities are;
//! - a hostile force with a body within [`SETTLER_HOLD_RADIUS`] of it adds
//!   the Settler's value ([`settler_hostage_value`]: its cost plus
//!   `SETTLER_PREMIUM`) to its Destroy row, and a nearby Defend row no longer
//!   covers it.
//!
//! The Destroy row keeps its own requirement (`DESTROY_MARGIN` of the
//! raiders' strength) and its own doctrine: it engages only on a favourable
//! exchange and otherwise holds defensive ground. So the gene does not send a
//! Warrior against a Horseman. It brings the force the board can afford to
//! the raiders that stop the expansion, ahead of rows worth less, and it
//! leaves the Settler's movement and safety rules alone. With the gene off
//! nothing changes.

use super::*;

/// Tiles from a Settler in the field within which a hostile holds it. The
/// safe-step guard refuses a step that a raider can reach next turn, so a
/// Horseman (four moves on open ground) or an Archer (two moves and range two)
/// standing within four tiles is what kept the Settlers of the finding still.
/// It is also the radius in which the last fix session saw raiders hold them.
pub(super) const SETTLER_HOLD_RADIUS: i32 = 4;

impl AdvancedAi {
    /// The tiles of our Settlers standing outside a city, which a raider can
    /// hold. Empty with the gene off.
    pub(super) fn settlers_in_the_field(&self, g: &Game, pid: usize) -> Vec<Pos> {
        if !self.raiders_holding_a_settler_are_hunted {
            return Vec::new();
        }
        g.player_unit_ids(pid)
            .into_iter()
            .map(|uid| &g.units[&uid])
            .filter(|unit| unit.kind == "settler" && g.city_at(unit.pos).is_none())
            .map(|unit| unit.pos)
            .collect()
    }
}

/// Whether a hostile at `at` holds one of the Settlers on `settlers`.
pub(super) fn holds_a_settler(g: &Game, at: Pos, settlers: &[Pos]) -> bool {
    settlers
        .iter()
        .any(|settler| g.wdist(*settler, at) <= SETTLER_HOLD_RADIUS)
}

/// The value the Settlers held by `force` add to its Destroy row: each
/// Settler with a body of the force within [`SETTLER_HOLD_RADIUS`] adds its
/// cost plus `SETTLER_PREMIUM`, the value of its own Escort row.
pub(super) fn settler_hostage_value(g: &Game, force: &[u32], settlers: &[Pos]) -> f64 {
    let held = settlers
        .iter()
        .filter(|settler| {
            force
                .iter()
                .any(|uid| g.wdist(g.units[uid].pos, **settler) <= SETTLER_HOLD_RADIUS)
        })
        .count();
    if held == 0 {
        return 0.0;
    }
    let cost = g.rules.units.get("settler").map_or(0.0, |spec| spec.cost);
    held as f64 * (cost + objective_board::SETTLER_PREMIUM)
}
