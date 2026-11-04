//! `science-specializes-earlier`: an assigned Science lane leaves the
//! development half at a third of the clock, as an assigned Culture lane
//! already does, instead of halfway.
//!
//! With an assigned lane, the grand strategy reads Expansion while the empire
//! holds fewer cities than it wants and a site remains, until
//! `phase_specialization_active` turns on at half the clock (turn 125 of
//! 250). Expansion weighs a beaker at 1.2 against the Science lane's 4.2,
//! names the Commercial Hub its district and the Merchant its Great Person,
//! and takes the expansion government and policy lists.
//! `campus-through-expansion` already gives a Science seat's Campus family the
//! lane's weights inside Expansion; everything else stays on Expansion's.
//!
//! Measured on the live Immortal 6p Continents seat (2026-11-02/03, 31
//! games):
//! - The seat never reached its city target (8-10 wanted, a median of 5 held
//!   at t100), so Expansion lasted to the halfway clock: 55% of the logged
//!   strategy turns. Every decided game was lost to a rival's science victory
//!   at t188-215.
//! - Even the best game was level on techs at t100 but six behind at t150. It
//!   built its Spaceport at t170 and finished its fourth launch at t214; the
//!   winner was done at t215.
//! - Religion seats that lost the Prophet race were handed to Science at
//!   about t40 and then waited in Expansion to t125 too.
//!
//! The Culture lane's tourism buildup has started at the first third since it
//! was written. Under this gene the Science lane's starts there as well. The
//! clock and every other lane are unchanged; off, nothing moves.

use super::*;

impl AdvancedAi {
    /// See the module: an assigned Science lane past the first third of the
    /// game clock. False with the gene off, for any other lane, or on an
    /// unclocked game.
    pub(super) fn science_buildup(&self, g: &Game) -> bool {
        self.science_specializes_earlier
            && self.victory_target == Some(VictoryTarget::Science)
            && g.max_turns > 0
            && g.turn.saturating_mul(3) >= g.max_turns.min(g.game_speed.turn_limit())
    }
}

#[cfg(test)]
mod tests;
