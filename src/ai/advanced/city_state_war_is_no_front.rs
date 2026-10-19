//! `city-state-war-is-no-front`: for a Domination seat, a war with a
//! city-state is no front the campaign target is chosen among
//! (`wartime_rivals` in `assess`). Domination counts foreign original
//! capitals; a city-state war we were dragged into (a suzerain joining its
//! major's war, a levy) otherwise names the city-state the plan's target,
//! and while any war runs `wartime_rivals` is non-empty, which also shuts
//! every at-peace branch of the target choice: the Domination frontier, the
//! prey and capital fronts, the denial counter and the capital pick. The
//! defence of a threatened city (`emergency_objective`) still comes first.
//!
//! **Why (live Emperor, 2026-10-15..19).** Of 7,874 campaign turns between
//! turns 60 and 200 in 120 games, 2,309 (29%) aimed at a city-state, and in
//! 43 games over 30% of them did -- civvis-20261018T122747Z alternated La
//! Venta and Geneva every turn from 73 to 79; 112557Z aimed at Hattusa and
//! La Venta inside a 75-turn war on the Khmer, who won a Culture Victory at
//! turn 150 with no city of theirs taken.
//!
//! Off, or for any other target, nothing changes.

use super::*;

impl AdvancedAi {
    /// `enemies` less every city-state, for a Domination seat with the gene
    /// on; unchanged otherwise.
    pub(super) fn without_city_state_war(&self, g: &Game, mut enemies: Vec<usize>) -> Vec<usize> {
        if self.city_state_war_is_no_front
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
        {
            enemies.retain(|enemy| g.players.get(*enemy).is_some_and(|player| !player.is_minor));
        }
        enemies
    }
}

#[cfg(test)]
mod tests;
