//! A Science seat's missing Campus unlock precedes optional research detours.
//!
//! `campus-unlock-every-lane` supplies Writing behind Astrology and luxury
//! connections. The earlier `campus-unlock-before-the-luxury` arm applies only
//! to Domination. A captured two-city Korea Science observation at turn 18
//! selected Irrigation with that switch both off and on. This separate opt-in
//! takes the earlier slot for Science, without moving the urgent goals above it.
//! `science-writes-before-it-sails` opens the same slot from the first city once
//! Pottery is known.

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

impl AdvancedAi {
    pub(super) fn science_writing_before_the_luxury_goal(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<&'static str> {
        let player = &g.players[pid];
        let cities = g.player_city_ids(pid).len();
        // `science-writes-before-it-sails` (see `advanced/research.rs`) takes
        // this slot from the first city once Pottery is in: live Deity Korea
        // founds its second city ~t48, after the Writing this gate exists to
        // hurry, so the two-city rule left Irrigation's luxury ahead of it.
        let one_city_writes = self.science_writes_before_it_sails
            && cities >= 1
            && player.techs.contains(&crate::name!("pottery"));
        ((self.science_writing_before_the_luxury && cities >= 2) || one_city_writes)
            .then_some(())
            .filter(|_| self.active_victory_target(g) == Some(VictoryTarget::Science))
            .filter(|_| !player.techs.contains(&crate::name!("writing")))
            .map(|_| "writing")
    }
}

#[cfg(test)]
mod tests;
