//! `tribute-buys-the-friendship`: gift Gold to the strongest neighbour at peace
//! while it is too cold to accept a friendship, up to the opinion a Friendly
//! rival holds, so the friendship ask can land.
//!
//! *If the enemy is superior in strength, evade him… the supreme art of war is
//! to subdue the enemy without fighting* (Sun Tzu, I and III).
//!
//! ## Why
//!
//! Live Deity Korea Science seats, 2026-11-04 21Z to 2026-11-05 19Z:
//! - A declared friend declared war on us on 0.2% of rival-turns within ten
//!   turns. Friendly: 3.5%, Unfriendly: 3.7%. The friendship is the guard, not
//!   the mood. Of 78 wars declared on us, the attacker was Unfriendly 53 times
//!   beforehand, Denounced 18 and Friendly 7, never a declared friend.
//! - The friendship ask (`befriend-the-strongest` and
//!   `friendship-keeps-the-peace`) crosses only to a rival the host reads as
//!   Friendly. 30 of those 50 asks were accepted within three turns. No
//!   Unfriendly rival is ever asked.
//! - The opinion breakdown (#4587, `rival.opinion_modifiers`) puts the median
//!   Unfriendly rival at −8 total and the median Friendly one at +4.
//! - A favorable deal moves it. On 173830Z at t130 a luxury bought from the
//!   Maori showed "Favorable trade deals to them" +10, decaying one point per
//!   two turns (+5 at t139). That is `STANDARD_DIPLOMATIC_ONE_SIDED_TRADES`
//!   (`Base/Assets/Gameplay/Data/Leaders.xml:1010`, `TRAIT_LEADER_MAJOR_CIV`,
//!   `ON_DEAL_ENACTED`: `TradeValuePerModifierPoint` 10, `MaxValue` 20,
//!   `ReductionTurns` 2).
//! - The engine does not model that modifier (`Game::do_trade`), so this gene
//!   is live-only: a sim screen cannot price it.
//!
//! First live carrier, 222639Z: 80 Gold to France at t115 took its visible
//! opinion from −3 to +7 on that turn, the row read +7 at t119, and the
//! friendship ask landed at t120. A luxury purchase (t118) and an embassy
//! (t119) helped too.
//!
//! ## What the gene does
//!
//! Inert with the flag off. On, the live bridge
//! (`civvis_orders::append_tribute_order`) picks the strongest met major at
//! peace with us, the rival `befriend-the-strongest` courts. It must be
//! stronger than us, read Unfriendly or Neutral, and carry an opinion
//! breakdown. On that rival's staggered cadence turn the bridge sends a
//! one-way Gold gift: the shipped `aid_gift` deal path with verb `TRIBUTE`, a
//! PROPOSED working deal holding only our Gold.
//! - The amount is ten Gold per point of the gap between the rival's visible
//!   total and the Friendly median, within the +20 the modifier holds.
//! - A treasury reserve is kept back, and a gift under the minimum never goes
//!   out.
//! - It holds for any other diplomatic or trade order to that rival this
//!   frame (the host keeps one working deal per rival).
//!
//! Once the rival reads Friendly, the friendship genes ask as they already
//! do. `Kind::OptIn`.
use super::*;

impl AdvancedAi {
    /// Whether `tribute-buys-the-friendship` is on, for the live bridge.
    pub fn tribute_buys_the_friendship_enabled(&self) -> bool {
        self.tribute_buys_the_friendship
    }
}

#[cfg(test)]
mod tests;
