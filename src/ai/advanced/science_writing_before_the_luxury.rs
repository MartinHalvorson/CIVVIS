//! A Science seat's missing Campus unlock precedes optional research detours.
//!
//! `campus-unlock-every-lane` supplies Writing behind Astrology and luxury
//! connections. The earlier `campus-unlock-before-the-luxury` arm applies only
//! to Domination. A captured two-city Korea Science observation at turn 18
//! selected Irrigation with that switch both off and on. This separate opt-in
//! takes the earlier slot for Science, without moving the urgent goals above it.
//!
//! Korea also takes this slot with its first city once Pottery is known. Waiting
//! for city two can commit an optional beeline just before that city is founded:
//! native 090547 chose Sailing at 16/0, founded city two later in that decision,
//! and did not finish Writing until 33. The earlier opening stays unchanged.
//! `science-writes-before-it-sails` opens the same first-city slot for any
//! Science seat once Pottery is known.

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

impl AdvancedAi {
    pub(super) fn science_writing_before_the_luxury_goal(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<&'static str> {
        if !(self.science_writing_before_the_luxury || self.science_writes_before_it_sails)
            || self.active_victory_target(g) != Some(VictoryTarget::Science)
            || g.players[pid].techs.contains(&crate::name!("writing"))
        {
            return None;
        }
        let cities = g.player_city_ids(pid).len();
        let pottery = g.players[pid].techs.contains(&crate::name!("pottery"));
        let korea_after_pottery = self.science_writing_before_the_luxury
            && cities == 1
            && g.players[pid].civ == "Korea"
            && pottery;
        // `science-writes-before-it-sails` (see `advanced/research.rs`): any
        // Science seat takes this slot from its first city once Pottery is in.
        // Live Deity founds its second city ~t48, after the Writing this gate
        // exists to hurry, and the seat is no longer always Korea.
        let one_city_writes = self.science_writes_before_it_sails && cities >= 1 && pottery;
        ((self.science_writing_before_the_luxury && cities >= 2)
            || korea_after_pottery
            || one_city_writes)
            .then_some("writing")
    }
}

#[cfg(test)]
mod tests;
