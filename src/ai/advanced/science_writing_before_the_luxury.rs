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

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

impl AdvancedAi {
    pub(super) fn science_writing_before_the_luxury_goal(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<&'static str> {
        if !self.science_writing_before_the_luxury
            || self.active_victory_target(g) != Some(VictoryTarget::Science)
            || g.players[pid].techs.contains(&crate::name!("writing"))
        {
            return None;
        }
        let cities = g.player_city_ids(pid).len();
        let korea_after_pottery = cities == 1
            && g.players[pid].civ == "Korea"
            && g.players[pid].techs.contains(&crate::name!("pottery"));
        (cities >= 2 || korea_after_pottery).then_some("writing")
    }
}

#[cfg(test)]
mod tests;
