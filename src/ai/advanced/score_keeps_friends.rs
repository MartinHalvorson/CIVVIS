//! `score-seat-keeps-friends` (opt-in): a Score seat accepts every declared
//! friendship a major offers it and offers one to every met major the host
//! lets it, nearest first.
//!
//! **Why (live Deity Rome, Score target, 6-player Continents, Online).** A
//! census of 94 live declarations on our seats found none while a declared
//! friendship or alliance stood (0 in about 4,500 rival-turns at peace),
//! against 0.31 per 100 rival-turns when the rival read Unfriendly and 0.71
//! when it had denounced us. The Score seat loses Rome or collapses in about
//! 3 games of 10 after a neighbour declares, each costing 250-400 points.
//! Yet tonight's Rome runs refused 52 friendships the AI offered
//! (`diplomacy_choice` rows `DECLARE_FRIEND`, initiator the rival,
//! `CHOICE_NEGATIVE` then `HUMAN_REFUSE_DEAL`): the control mod answers a
//! leader's statement in `CivvisControlAutoClose.lua` with a NEGATIVE-first
//! ladder, and only the delegation and embassy visits were ever welcomed.
//! The seat's own offers already land (185 accepted the same nights), but
//! `befriend-the-strongest` asks one rival every third turn, the strongest
//! first.
//!
//! **The gene.** While the active victory target is Score:
//!
//! 1. **Accept.** Every order batch renews a one-turn lease in the dialogue
//!    closer (`LuaEvents.CivvisDialogueKeepsFriends`, carried by a
//!    `combat_policy` order `SCORE_SEAT_KEEPS_FRIENDS`). Under it a major's
//!    `DECLARE_FRIEND` offer is answered with its `CHOICE_POSITIVE`
//!    (`DiplomacyStatements_DeclareFriendship.xml:82-83`,
//!    `LOC_DIPLO_DECLARE_FRIEND_CHOICE_POSITIVE`), except from the rival whose
//!    city `score-seat-takes-what-pays` names as the campaign's
//!    ([`AdvancedAi::score_keeps_friends_spares`]), which keeps the refusal.
//!    The closer reports `"keeps_friends":"accept"` or `"spared"` on its
//!    `diplomacy_choice` row.
//! 2. **Offer.** `befriend-the-strongest`'s offer runs every turn instead of
//!    every third, with or without that gene, and asks the nearest met major
//!    first (fewest tiles between any two of our cities), then the strongest.
//!    The host still befriends only from Friendly, at peace and with no deal
//!    pending, so a rival is asked as soon as it allows it, a lapsed friend
//!    first (`friendship-keeps-the-peace`'s renewal). Never the campaign's
//!    target and never a rival at war with us.
//!
//! No AI-offered alliance appeared in the same logs (the only alliance rows
//! are our own proposals), so nothing here answers one. The seat's denounces
//! on those nights all came from `launcher-war-serves-every-lane` before its
//! counter to a rival's space launch, which this gene leaves alone.
//!
//! Off, or for another victory target, nothing changes. Grep the journal
//! for `score-seat-keeps-friends:`.

use super::*;

impl AdvancedAi {
    /// The gene is on and the seat's active target is Score.
    pub(super) fn score_keeps_friends_on(&self, g: &Game) -> bool {
        self.score_seat_keeps_friends && self.active_victory_target(g) == Some(VictoryTarget::Score)
    }

    /// The rival whose city `score-seat-takes-what-pays` names as the
    /// campaign's, which is neither befriended nor accepted. `None` when no
    /// city is named, with that gene off and off the Score lane.
    pub(crate) fn score_keeps_friends_spares(&self, g: &Game) -> Option<usize> {
        if !self.score_take_names_a_city(g) {
            return None;
        }
        self.score_take_state.named.map(|(owner, _, _)| owner)
    }

    /// `score-seat-keeps-friends`: the live dialogue closer's lease, and the
    /// native player it spares. `None` with the gene off or for another
    /// target.
    pub fn score_seat_keeps_friends_lease(&self, g: &Game) -> Option<Option<usize>> {
        self.score_keeps_friends_on(g)
            .then(|| self.score_keeps_friends_spares(g))
    }

    /// Fewest tiles between a city of ours and a city of `other`; `i32::MAX`
    /// when either holds none.
    pub(super) fn score_friend_distance(g: &Game, pid: usize, other: usize) -> i32 {
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        g.player_city_ids(other)
            .into_iter()
            .flat_map(|cid| {
                let theirs = g.cities[&cid].pos;
                ours.iter().map(move |pos| g.wdist(*pos, theirs))
            })
            .min()
            .unwrap_or(i32::MAX)
    }
}

#[cfg(test)]
mod tests;
