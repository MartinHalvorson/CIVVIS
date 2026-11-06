//! `stronger-racer-stays-a-friend`: on a Science seat, a rival stronger than us
//! that is not yet in the space race can be befriended even while it reads as
//! racing.
//!
//! *If the enemy is superior in strength, evade him* (Sun Tzu, I).
//!
//! `friendship-ends-at-the-race` holds a rival out of friendship from a race
//! reading of `FRIENDSHIP_RACE_CEILING`, so the counter can reach it. On the
//! Deity Science lane that reading comes long before any launch.
//!
//! Live Deity seats since 2026-11-05 19:07Z (29 games):
//! - Rivals reading Friendly and stronger than us stood at peace for 994
//!   rival-turns. We asked for a friendship within three turns on 8% of them,
//!   and those rivals declared war on us within ten turns on 5.5-7.9%, against
//!   0.2% for a declared friend.
//! - Replaying 18 of those boards with one friendship gate removed at a time,
//!   removing `friendship-ends-at-the-race` freed the ask on 15, and no other
//!   gate freed any.
//! - Of 44 friendships that rule let lapse, we declared war on none within 20
//!   turns.
//!
//! The rule still matters once a launch is real. Our one Deity win (122513Z)
//! declared on Georgia at t185, "past its Moon with a pad standing", which a
//! friendship would have barred. So a rival with a project landed or a pad we
//! have seen stays out, as does one the counter reads as urgent or whose
//! culture race is near its finish (King seat 30700004 lost to Gaul's Culture
//! with the counter barred by a friendship).
//!
//! Off: unchanged.
use super::protective_friendship::FRIENDSHIP_CULTURE_HORIZON;
use super::*;

impl AdvancedAi {
    /// Whether `rival`, though racing, may still be befriended: the gene on, a
    /// Science aim, stronger than us, not physically in the space race, not
    /// an urgent victory threat, and no culture race near its finish.
    pub(super) fn racer_stays_a_friend(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.stronger_racer_stays_a_friend
            && (self.victory_target == Some(VictoryTarget::Science)
                || self
                    .plan
                    .as_ref()
                    .is_some_and(|plan| plan.strategy == GrandStrategy::Science))
            && g.military_power(rival) > g.military_power(pid)
            && g.players[rival].science_projects.is_empty()
            && Self::science_denial_pad(g, pid, rival).is_none()
            && !self.urgent_victory_threat(g, rival)
            && !self
                .projected_culture_finish(g, rival)
                .is_some_and(|turns| turns <= FRIENDSHIP_CULTURE_HORIZON)
    }
}

#[cfg(test)]
mod tests;
