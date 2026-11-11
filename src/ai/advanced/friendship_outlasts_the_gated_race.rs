//! `friendship-outlasts-the-gated-race`: a lapsed friend racing to win is
//! renewed while `denial-war-needs-the-lane-edge` would refuse the war on it.
//!
//! `friendship-ends-at-the-race` lets a declared friendship lapse unrenewed
//! once the friend is racing to win, so that the counter, a war, can be
//! declared on it. `denial-war-needs-the-lane-edge` refuses that war while
//! our military is under the lane edge (1.2 times the rival's steady power,
//! 1.4 on a Diplomatic seat). On such a seat the lapse buys no counter and
//! only removes the shield: a declared friend cannot declare war on us.
//!
//! Live Deity Online Kristina (Culture), 2026-11-10/11: the race rule ended
//! 26 friendships across the day's Culture games. In civvis-20261110T202929Z
//! it ended Georgia's at t134 ("they are racing to win, so they are left to
//! the counter"; its Diplomatic race read 50%) at 352 power against 383. We
//! never declared on Georgia; Georgia declared on us at t145 and took
//! Stockholm at t149. No declared friend declared war on us in the day's 34
//! declarations.
//!
//! With the gene and `denial-war-needs-the-lane-edge` both on, a friend that
//! lapsed within `FRIENDSHIP_RENEW_WINDOW` turns (so `friendship-keeps-the-peace`
//! and `befriend-the-strongest` are on too) is no longer held back by the
//! race when our power is under the gate's edge against it: the renewal is
//! asked as for any other lapsed friend, and one journal line names the race
//! and the power. At or over the edge, or on a Domination seat, the race
//! still ends the friendship. With either gene off nothing changes.

use super::*;

impl AdvancedAi {
    /// `Some((ours, theirs, edge))` when this gene keeps `other`'s lapsed
    /// friendship through `friendship-ends-at-the-race`: both genes on,
    /// `other` lapsed within the renewal window and racing, and our power
    /// under the denial gate's edge against it. `None` otherwise, and always
    /// with either gene off.
    pub(super) fn gated_race_keeps_the_friend(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
        lapsed: &BTreeSet<usize>,
    ) -> Option<(f64, f64, f64)> {
        if !self.friendship_outlasts_the_gated_race
            || !self.friendship_ends_at_the_race
            || !lapsed.contains(&other)
            || !self.friend_is_racing(g, other)
        {
            return None;
        }
        self.under_the_denial_lane_edge(g, pid, other)
    }

    /// The journal line for a renewal this gene made possible.
    pub(super) fn journal_gated_race_renewal(
        &self,
        g: &Game,
        partner: usize,
        (ours, theirs, edge): (f64, f64, f64),
    ) {
        think!(self.journal(), Diplomacy, Decision,
               "Keeping the friendship with {}", g.players[partner].civ;
               "friendship-outlasts-the-gated-race: they are racing to win, but a war on them \
                needs {edge:.1} times their steady power; {ours:.0} against {theirs:.0}");
    }
}

#[cfg(test)]
mod tests;
