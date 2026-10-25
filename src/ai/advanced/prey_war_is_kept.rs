//! `prey-war-is-kept`: a Domination seat at war with a major rival at or
//! under [`PREY_DECLARE_RATIO`] of our military power keeps that war and
//! fights it.
//!
//! - **The war is kept.** No white peace is offered to that rival by the
//!   faith war's exit, a Recovery reading, a stalled or fatigued war, a
//!   banked-envoy reclaim, a finished interception, or the one-war rule's
//!   second front or rout. Peace is still offered when they outmatch us, when
//!   a city of ours is falling, and when the one-war rule makes it for the
//!   victory counter or a capital already settled.
//! - **The war is a front.** A faith war on prey whose city we have located
//!   is not exempted by `faith-war-is-no-front`: the campaign target is chosen
//!   among it like any other war.
//!
//! **Why (live Emperor, 29 games of 2026-10-25).** In 5 games the seat made
//! peace with a rival at a seventh to a third of its power:
//! - China, 444 against 63, turn 87 (civvis-20261025T055158Z);
//! - Australia, 765 against 69, turn 179 (043224Z);
//! - Germany, 848 against 111, turn 219 (032555Z);
//! - Portugal, 1053 against 234, turn 182 (030933Z);
//! - the Aztec, 334 against 100, turn 104 (014329Z).
//!
//! In 055158Z China's capital Xi'an stood at walls 0 and 40 HP at turn 83
//! with one of ours five tiles away. `faith-war-is-no-front` moved the
//! campaign onto Greece, the faith war's exit offered China peace at 86,
//! Greece took Xi'an, and the faith war then opened on Greece at 434 against
//! 427 cost us 8 cities.
//!
//! Off, nothing changes.

use super::prey_declaration::PREY_DECLARE_RATIO;
use super::*;

impl AdvancedAi {
    /// Whether the war with `rival` is kept: the gene is on, the seat plays
    /// for Domination, `rival` is a living major at war with us at or under
    /// [`PREY_DECLARE_RATIO`] of our power, and no city of ours is falling.
    pub(super) fn prey_war_is_kept_with(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.prey_war_is_kept
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.players
                .get(rival)
                .is_some_and(|player| player.alive && !player.is_minor && !player.is_barbarian)
            && g.is_at_war(pid, rival)
            && g.military_power(rival) <= PREY_DECLARE_RATIO * g.military_power(pid)
            && !Self::our_cities_falling(g, pid)
    }

    /// Whether `rival`'s faith war is still a front: it is prey whose war is
    /// kept, and we have located a city of its to aim at.
    pub(super) fn prey_faith_war_is_a_front(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.prey_war_is_kept_with(g, pid, rival) && !g.player_city_ids(rival).is_empty()
    }

    pub(super) fn journal_prey_war_is_kept(&mut self, g: &Game, pid: usize, rival: usize) {
        if !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        let ours = g.military_power(pid);
        let theirs = g.military_power(rival);
        think!(self.journal(), Diplomacy, Decision,
               "Keeping the war on {}", g.players[rival].civ;
               "prey-war-is-kept: {theirs:.0} power against our {ours:.0}, at or under {PREY_DECLARE_RATIO}, and no city of ours is falling");
    }
}

#[cfg(test)]
mod tests;
