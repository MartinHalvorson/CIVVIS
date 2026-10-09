//! `peacetime-classical-republic`: the first government is Classical Republic
//! while no major is at war with us.
//!
//! **The finding (115 Emperor runs of 2026-10-07/09, pins fb5ac2506 onward).**
//! Political Philosophy lands at a median turn 41 and Divine Right at 80. In
//! that 40-turn window the conquest lane list (`government_priorities`) names
//! no tier-one government but Oligarchy, so the seat ran Oligarchy on 4,672 of
//! 4,780 window turns. Oligarchy's +4 strength is worth something only at war,
//! and we were at war with a major on a mean 15% of the window (median 0%).
//! The same window is where the gate is decided: runs with 7 or more cities at
//! turn 75 pass the turn-150 production gate 57% of the time, 5-6 cities 10%.
//! Our cities are housing-bound (54% within one citizen of their housing at
//! turns 50-75, growing at about 40% of their rate) and short of Amenities
//! (27-31%). The rivals run Classical Republic: 155 of the rival seats at turn
//! 50, ours none.
//!
//! **The mechanism.** Classical Republic gives +1 Housing and +1 Amenity to
//! every city with a specialty district (42-69% of our cities at turns 45-75;
//! 25-36% of all our cities are both housing-bound and hold one) and seats two
//! Economic cards (Colonization's +50% Settler production beside Urban
//! Planning) where Oligarchy seats one.
//!
//! **The gene.** When the lane's pick is Oligarchy (no tier-two government is
//! unlocked yet), Classical Republic is unlocked, we are not already under
//! Oligarchy, and no living major is at war with us, the pick becomes Classical
//! Republic. At war the lane's Oligarchy stands: a first switch to it is free,
//! and the Anarchy guard in `strategic_government` then keeps it (a return to
//! the Republic is an equal-capacity repeat). Divine Right's Monarchy still
//! replaces either on capacity. Off, the choice is unchanged.

use super::AdvancedAi;
use crate::game::Game;
use crate::think;

impl AdvancedAi {
    /// `peacetime-classical-republic`: the government `strategic_government`
    /// should pursue given the lane's `choice`. Returns `choice` unchanged
    /// with the gene off, when the pick is not Oligarchy, when Classical
    /// Republic is still locked, when the seat already runs Oligarchy, or
    /// while a living major is at war with `pid`.
    pub(super) fn peacetime_republic_choice(
        &self,
        g: &Game,
        pid: usize,
        choice: Option<String>,
    ) -> Option<String> {
        if !self.peacetime_classical_republic || choice.as_deref() != Some("oligarchy") {
            return choice;
        }
        let player = &g.players[pid];
        if player.government.as_deref() == Some("oligarchy") {
            return choice;
        }
        let republic_unlocked = g
            .rules
            .governments
            .get("classical_republic")
            .is_some_and(|spec| {
                spec.civic
                    .as_ref()
                    .is_none_or(|civic| player.civics.contains(civic))
            });
        if !republic_unlocked {
            return choice;
        }
        let war = g.players.iter().find(|rival| {
            rival.id != pid
                && rival.alive
                && !rival.is_minor
                && !rival.is_barbarian
                && g.is_at_war(pid, rival.id)
        });
        if let Some(rival) = war {
            think!(self.journal(), Government, Detail,
                   "Oligarchy for the war";
                   "at war with player {}: Oligarchy's strength outweighs the \
                    Republic's Housing and Amenity", rival.id);
            return choice;
        }
        think!(self.journal(), Government, Detail,
               "Classical Republic while at peace";
               "no major is at war with us: +1 Housing and +1 Amenity in every \
                city with a district and a second Economic card, ahead of \
                Oligarchy's strength");
        Some("classical_republic".to_string())
    }
}

#[cfg(test)]
mod tests;
