//! `stalled-conquest-takes-science`: a Domination-target seat that holds no
//! rival's original capital by [`STALLED_CONQUEST_STANDARD`] plays Science.
//!
//! A Domination victory needs every rival capital. Live King, 11-01/02
//! (random-leader 6p Continents, the culture / religious / domination roll):
//! - Domination seats won 0 of 13 decided games and lost 8 to Science and 5
//!   to Culture, at a median tech gap of −5 at t150.
//! - Religion seats won 6 of 20, every one by Science, at a median gap of −6:
//!   `religion-lane-hands-over` retargets a seat that lost the Prophet race
//!   (closed near t47) to Science, so it plays the Science lane from then on.
//! - Of the 9 Domination seats with traces, 8 held no rival capital at t150
//!   and 9 held none at t100; the first capitals fell at t103, t172-211 or
//!   never, and none of those seats won.
//!
//! With the gene on, once the standard clock passes
//! [`STALLED_CONQUEST_STANDARD`] (live turn 120 at Online speed, about a
//! hundred turns before our Science wins of t207-249) and no city of ours is
//! a rival major's original capital, the seat retargets to Science (else
//! Culture). The retarget is permanent. Off, nothing changes.

use super::*;

/// Standard turns after which a capital-less conquest hands over: live turn
/// 120 at the seat's Online speed.
pub(crate) const STALLED_CONQUEST_STANDARD: u32 = 180;

impl AdvancedAi {
    /// Whether any city of ours is a rival major's original capital. The
    /// engine keeps `is_capital` on a captured capital with its
    /// `original_owner` (the Domination check in `Game::transfer_city` reads
    /// capitals the same way).
    pub(super) fn holds_a_rival_capital(g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid).into_iter().any(|cid| {
            let city = &g.cities[&cid];
            city.is_capital
                && city.original_owner != pid
                && g.players
                    .get(city.original_owner)
                    .is_some_and(|owner| !owner.is_minor && !owner.is_barbarian)
        })
    }

    /// The lane a stalled conquest takes: Science, else Culture.
    fn stalled_conquest_lane(g: &Game) -> Option<VictoryTarget> {
        [VictoryTarget::Science, VictoryTarget::Culture]
            .into_iter()
            .find(|lane| g.victory_conditions.is_enabled(lane.as_str()))
    }

    /// See the module. Runs once a turn, before the turn reads its target.
    /// A no-op with the gene off, on any other target, before the standard
    /// turn, or while a rival capital is ours.
    pub(super) fn hand_over_a_stalled_conquest(&mut self, g: &Game, pid: usize) {
        if !self.stalled_conquest_takes_science
            || self.victory_target != Some(VictoryTarget::Domination)
            || g.turn < g.standard_duration(STALLED_CONQUEST_STANDARD)
            || Self::holds_a_rival_capital(g, pid)
        {
            return;
        }
        let Some(lane) = Self::stalled_conquest_lane(g) else {
            return;
        };
        think!(self.journal(), Strategy, Strategy,
               "Handing the Domination lane to {}", lane.as_str();
               "stalled-conquest-takes-science: turn {} and no rival capital is ours; a Domination victory needs every one",
               g.turn);
        self.retarget(lane);
    }
}

#[cfg(test)]
mod tests;
