//! `tribute-and-friendship-land`: the tribute is priced at what a Gold gift
//! actually buys in the world's era, and the friendship ask reaches a
//! Friendly rival alone and again at the mod's own pace, so the rival one gift
//! or one ask could warm is warmed before its war.
//!
//! ## Why
//!
//! Live Lv 8 Deity games, 2026-11-19 12:00Z to 11-20 09:30Z (121 runs, both
//! `tribute-buys-the-friendship-2` and `tribute-warms-the-neighbour` armed in
//! every one). Of 27 wars on us where the attacker was within one gift's
//! reach or read Friendly before its war, read frame by frame:
//! - In ten the attacker had been sent a tribute that landed short: the gift
//!   bought fewer points than it was priced at, the rival sat at +3 or +4 and
//!   stayed Unfriendly, and every top-up the bridge ordered afterwards was
//!   refused by the mod's 30-turn bar (`aid_gift_cooldown`).
//! - In six the attacker read Friendly for 5 to 23 turns and the friendship
//!   order was withheld for most of them by the bridge's refused-order rule
//!   (three `not_friends` failures withhold an order for ten turns). The
//!   three failures are one real ask and the two turns after it, which the
//!   mod itself refuses inside its five-turn `PeaceRetryTurns` window, so a
//!   Friendly rival is asked once every thirteen turns, not every five.
//! - In two the only ask shared its frame with a delegation or embassy to
//!   the same rival and never reached the rival.
//!
//! What a gift buys: 178 of 202 tributes enacted. Where the rival held no
//! favorable-deal row, the opinion bought per ten Gold, by the world era the
//! frame reports (`world_era`, 0 the Ancient era), had medians 0.60 (era 0,
//! 14 gifts), 0.67 (era 1, 39), 0.78 (era 2, 38) and 0.89 (era 3 and later,
//! 95). Version two prices every point at ten Gold, less a point
//! (`TRIBUTE_TARGET_OPINION_2`), which holds from the fourth era on. Earlier
//! it does not: 80 Gold bought +5, 90 bought +6, 100 bought +6 or +7. Gifts
//! to an Unfriendly rival left it under +5 in 11 of 13 at era 0, 20 of 38 at
//! era 1, 7 of 37 at era 2 and 2 of 89 later. Under +5 they bought nothing:
//! 1 of 47 turned Neutral or better within three turns, against 16 of 25 at
//! +5, 57 of 68 at +6 and 26 of 29 at +7 or more. The era rate predicts the
//! points bought within one in 86% of gifts, against 70% for the flat rate.
//! The most any gift bought was +10.
//!
//! What a re-ask buys: 1,143 friendship asks reached a Friendly rival; 1,071
//! were accepted within two turns. An ask sharing its frame with a
//! delegation or embassy session to the same rival was accepted 3 times in
//! 32 (the agent opens both with `DiplomacyManager.RequestSession`, the
//! shipped call, `Base/Assets/UI/DiplomacyActionView.lua:473`, and the second
//! session takes the frame); without one, 95%. The next ask after a refused
//! one, to a rival still Friendly, was accepted 33 times in 43. 388
//! rival-turns in 38 of the 121 games read Friendly while the friendship
//! order was withheld.
//!
//! ## What the gene does
//!
//! Inert with the flag off. On, in the live bridge (`civvis_orders`):
//! - Both tributes (`tribute-buys-the-friendship[-2]`,
//!   `tribute-warms-the-neighbour`) price the opinion gap at
//!   [`TRIBUTE_ERA_GOLD_PER_POINT`] for the frame's world era (17, 15, 13,
//!   then 11 Gold a point) instead of ten, rounded up to whole tens. The
//!   rival, the +7 aim, the ten-point reach of one gift, the one gift per
//!   rival per 30 turns, and the whole-or-nothing rule are unchanged, so a
//!   gift is never split and never paid twice; a treasury short of the whole
//!   gift waits. A frame with no era reading keeps ten.
//! - A delegation or embassy order to a rival that this frame also asks for a
//!   friendship is dropped, so the friendship session goes alone.
//! - A friendship order is never withheld by the refused-order rule: the
//!   mod's five-turn window paces the asks, and refuses the frames between
//!   without asking.
//!
//! `friendship-only-with-the-stronger` never held an ask in those games
//! (no journal line in 121 runs), so who is asked is unchanged.
//! `Kind::OptIn`.
use super::*;

/// `tribute-and-friendship-land`: Gold one tribute pays per point of visible
/// opinion, by world era (0, 1, 2, then 3 and later): the inverse of the live
/// median bought per ten Gold, 0.60, 0.67, 0.78 and 0.89, rounded up.
pub const TRIBUTE_ERA_GOLD_PER_POINT: [f64; 4] = [17.0, 15.0, 13.0, 11.0];

impl AdvancedAi {
    /// Whether `tribute-and-friendship-land` is on, for the live bridge.
    pub fn tribute_and_friendship_land_enabled(&self) -> bool {
        self.tribute_and_friendship_land
    }

    /// `tribute-and-friendship-land`: the Gold a tribute pays per point of
    /// opinion in `world_era`, or `None` with the gene off or no era read
    /// (the bridge then keeps its ten).
    pub fn tribute_gold_per_point(&self, world_era: Option<i64>) -> Option<f64> {
        if !self.tribute_and_friendship_land {
            return None;
        }
        let era = world_era?.clamp(0, TRIBUTE_ERA_GOLD_PER_POINT.len() as i64 - 1);
        Some(TRIBUTE_ERA_GOLD_PER_POINT[era as usize])
    }
}

#[cfg(test)]
mod tests;
