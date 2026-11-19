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
//! The first version then paid every five turns, and partial amounts toward
//! any gap. On its first four carriers 14 gifts (1,392 Gold) warmed one rival,
//! so it was disarmed at 2026-11-05 23:34Z:
//! - A first gift bought about +9 to +10 (80, 101 and 139 Gold). A further 80
//!   or 120 Gold to the same rival bought about +2 and left the row near +8.
//! - Seven gifts (789 Gold) went to a rival more than the modifier's room from
//!   Friendly, and none warmed it. The −20 `Different Governments` row stood
//!   against us in all seven (once hidden as "Unknown Reason"), so a rival on a
//!   modern government is usually out of reach while we are not.
//!
//! So the gift is now one whole payment per rival per 30 turns, and only to a
//! rival one gift can reach.
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
//!   total and the Friendly median, sent whole. A gap over ten points is out
//!   of reach and no gift goes out.
//! - A rival holding a "Favorable trade deals" row is not paid again, and the
//!   mod refuses a second tribute to a rival inside `TributeRetryTurns` (30).
//! - A treasury reserve is kept back.
//! - It holds for any other diplomatic or trade order to that rival this
//!   frame (the host keeps one working deal per rival).
//!
//! Once the rival reads Friendly, the friendship genes ask as they already
//! do. `Kind::OptIn`.
//!
//! ## Version two: `tribute-buys-the-friendship-2`
//!
//! Live Deity games of 2026-11-18 (83 that reached turn 60, version one armed
//! in every one):
//! - A gift buys a point less than its Gold over ten: where the row was
//!   visible, 60 Gold read +5, 80 read +7 twice and 90 read +8. Version one
//!   aims at +5, so the rival lands on +4. The host must turn an Unfriendly
//!   leader Neutral only at +5 (`Base/Assets/Gameplay/Data/DiplomaticActions.xml`,
//!   `DiplomaticStateTransitions`: UNFRIENDLY→NEUTRAL `RequireTransitionMax`
//!   5), and +4 does not cross.
//! - Of 54 tributes, 39 left the rival at +4 or less over the gift turn and
//!   the next (2,530 of 3,450 Gold). 34 of those 39 stayed Unfriendly, and 8
//!   led to a declared friendship within ten turns. All 15 that reached +5
//!   turned Neutral or better within three turns, and 10 led to a friendship.
//! - The 100-Gold reserve held the gift on 1,075 bridge frames, against 131
//!   frames that sent one. The median treasury in the twelve turns before a
//!   war declared on us by turn 70 was 68 Gold.
//! - Replaying the rule on the logged frames, version one paid 4 of the 80
//!   rivals that later declared war on us before their war. Aimed at +7 from
//!   the whole treasury it would have paid 22.
//!
//! Version two aims at +7, the `tribute-warms-the-neighbour` aim, so the gift
//! clears the line with a point to spare for the row's decay. It may spend the
//! whole treasury, less Gold another gift took this frame, and sends nothing
//! while Gold per turn is negative. The rival, the cadence, the ten-point
//! reach and the one-gift-per-30-turns bar are version one's. Version two
//! carries version one: arming it replaces `tribute-buys-the-friendship`.
use super::*;

impl AdvancedAi {
    /// Whether `tribute-buys-the-friendship` or its version two is on, for the
    /// live bridge.
    pub fn tribute_buys_the_friendship_enabled(&self) -> bool {
        self.tribute_buys_the_friendship || self.tribute_buys_the_friendship_2
    }

    /// Whether `tribute-buys-the-friendship-2` is on: the bridge aims the
    /// tribute at +7 and keeps no Gold back.
    pub fn tribute_aims_past_the_line(&self) -> bool {
        self.tribute_buys_the_friendship_2
    }
}

#[cfg(test)]
mod tests;
