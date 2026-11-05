//! A Science seat's missing Campus unlock precedes optional research detours.
//!
//! `campus-unlock-every-lane` supplies Writing behind Astrology and luxury
//! connections. The earlier `campus-unlock-before-the-luxury` arm applies only
//! to Domination. A captured two-city Korea Science observation at turn 18
//! selected Irrigation with that switch both off and on. This separate opt-in
//! takes the earlier slot for Science, without moving the urgent goals above it.

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

impl AdvancedAi {
    pub(super) fn science_writing_before_the_luxury_goal(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<&'static str> {
        (self.science_writing_before_the_luxury
            && self.active_victory_target(g) == Some(VictoryTarget::Science)
            && g.player_city_ids(pid).len() >= 2
            && !g.players[pid].techs.contains(&crate::name!("writing")))
        .then_some("writing")
    }
}

#[cfg(test)]
mod tests;
