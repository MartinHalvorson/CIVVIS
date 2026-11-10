//! `space-denial-promotes-the-rocket-scientist` (opt-in): a Culture or
//! Domination seat whose spies answer a space race promotes them to Rocket
//! Scientist first.
//!
//! Rocket Scientist (`PROMOTION_SPY_ROCKET_SCIENTIST`): "Disrupt Rocketry as
//! if 2 levels more experienced". Host odds read live: Disrupt Rocketry for a
//! level-one Spy 0.25-0.37 against Deity pads on 11-10, 0.62 for a level-four
//! Spy on 11-05, about 0.085 a level.
//!
//! The spy loop in `diplomacy.rs` takes the first host-offered promotion on a
//! fixed list per plan. The Culture list is Cat Burglar, Con Artist,
//! Disguise, Linguist, Surveillance; the Conquest list License to Kill,
//! Demolitions, Guerrilla Leader, Covert Action, Ace Driver; neither names
//! Rocket Scientist. Live Deity Kristina, 11-10: the host offered Rocket
//! Scientist in 6 of 28 Spy promotion menus across 8 rival Science losses
//! (173750Z t159, t163, t171; 164254Z t178, t179; 165607Z t153) and was
//! taken in none (Quartermaster, Ace Driver, or the Spy was lost first).
//!
//! While [`AdvancedAi::denial_spy_racers`] names a rival and the seat aims at
//! Culture or Domination with the Science Victory on, Rocket Scientist goes
//! ahead of the plan's list; the rest of the list keeps its order. Science
//! and Religion seats keep theirs (the Science list already holds Rocket
//! Scientist second, behind Technologist). Exact no-op while the gene is off.

use super::*;

impl AdvancedAi {
    /// Whether this seat's Spies take Rocket Scientist first: the gene on, a
    /// Culture or Domination aim, the Science Victory on, and a racer the
    /// spy network answers.
    pub(super) fn denial_wants_the_rocket_scientist(&self, g: &Game, pid: usize) -> bool {
        self.space_denial_promotes_the_rocket_scientist
            && g.victory_conditions.science
            && matches!(
                self.active_victory_target(g),
                Some(VictoryTarget::Culture | VictoryTarget::Domination)
            )
            && !self.denial_spy_racers(g, pid).is_empty()
    }

    /// `priority` with Rocket Scientist moved to the front.
    pub(super) fn rocket_scientist_first<'a>(priority: &[&'a str]) -> Vec<&'a str> {
        std::iter::once("rocket_scientist")
            .chain(
                priority
                    .iter()
                    .copied()
                    .filter(|promotion| *promotion != "rocket_scientist"),
            )
            .collect()
    }
}

#[cfg(test)]
mod tests;
