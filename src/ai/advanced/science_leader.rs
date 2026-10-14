//! `science-leader-is-the-target`: at peace, the rival that out-researches
//! the board is the campaign's rival, from standard turn
//! [`SCIENCE_LEADER_TURN`] (turn 109 at the live seat's Online speed), long
//! before its first launch.
//!
//! The denial counter reads a rival's Science race off the public launch
//! ladder alone (`science_launch_progress`): 0 until the Earth Satellite
//! lands, 58 after it under `science-ladder-reads-the-clock`, and the
//! Domination counter needs 45 (`DOMINATION_SCIENCE_COUNTER`).
//! `science-chain-alarm`, the one pre-launch reading, ships off. So the
//! counter cannot see a Science race until the first project lands, 15 to 35
//! turns before the win. The Emperor control runs G185-G190
//! (civvis-20261006T024058Z .. T035131Z) lost to Science at turns 202-237;
//! "Countering <civ> | its science race reads N%" first appears at G187
//! turn 190 (the Maya, 67%: Moon Landing at 183) and G190 turn 198 (the
//! Khmer, 94%). The Zulu won G187 at 220 from a Satellite at 187, so at 190
//! they read 58 against the Maya's 67 and were never named.
//!
//! The winner was readable from turn 100: the rival with the most Science a
//! turn (the host's top-bar figure, which the mirror lands in
//! `observed_yield_adjustments`) was the Science winner in all five of
//! G185-G190 at turns 80, 100 and 120 -- and in 15 of the 23 Science losses
//! of October 5-6 at turn 110 (10-turn mean), against 9 of 23 for the
//! technology-count leader. Over all 88 losses with a reading it named the
//! eventual winner 43 times, against a 1-in-3 base rate.
//!
//! The pick is deliberately BEHIND the actionable denial. When the counter
//! named a rival at turns 110-130 (26 of 90 losses) that rival won 20
//! times; where it and the Science leader disagreed (15), the counter's
//! rival won 11 and the Science leader 3. So the leader replaces only the
//! elective choice -- the next capital's owner, the frontier, the required
//! capital, the city campaign and the value sort -- never a counter, a
//! Diplomatic Victory contender, a rush, a forced target or a war already
//! running: it re-aims the campaign the seat would open anyway rather than
//! opening an extra front.
//!
//! And it is only a target the army can beat. It must be reachable by land
//! (`rival_reachable_by_land`), hold a city within the declaration range,
//! be legal to target, and stand under our military
//! [`super::one_war::DECLARATION_EDGE_RATIO`] times over its steady reading
//! (`steady_rival_power`) -- the bar `urgent-denial-needs-the-edge` holds an
//! urgent clock to. The declaration itself keeps every gate it had: the
//! staged siege, the version-2 edge, the breaker wait. Over October 5-6 the
//! gates leave the Zulu of G187 (1,374 power to our 645 at turn 120) and
//! Egypt of G189 (575 to 240) out of reach; in all losses the pick fires at
//! peace with no counter standing in 15 of 90, and re-aims the campaign in
//! 7 of those.

use super::one_war::DECLARATION_EDGE_RATIO;
use super::{AdvancedAi, GrandStrategy, VictoryTarget};
use crate::game::Game;

/// Standard turn from which the Science leader is read: turn 109 at the
/// live seat's Online speed. The census turns above are Online turns; the
/// Science-a-turn leader is as predictive at 110 as at 120 (15 of 23
/// Science losses at both), and a war opened then has 60-90 turns before
/// the first launch of those games (turns 174-190).
pub(crate) const SCIENCE_LEADER_TURN: u32 = 165;

/// Science a turn the leader must make over every other major we have met,
/// ourselves included, to count as a clear lead. At 1.15 the 10-turn mean
/// reading named a leader in 21 of the 23 Science losses at turn 120.
pub(crate) const SCIENCE_LEADER_MARGIN: f64 = 1.15;

impl AdvancedAi {
    /// A seat's Science a turn: its cities, its player-level extras, and the
    /// host's correction for that seat. On the live board a rival's cities
    /// are rebuilt from what we have seen, and its public Science figure
    /// lands in `observed_yield_adjustments` as the difference, so the sum is
    /// the host's figure.
    pub(crate) fn seat_science_per_turn(g: &Game, seat: usize) -> f64 {
        g.player_city_ids(seat)
            .into_iter()
            .map(|city| g.city_yields(city).science)
            .sum::<f64>()
            + g.player_yield_extras(seat).science
            + g.observed_yield_adjustments
                .get(&seat)
                .map_or(0.0, |adjustment| adjustment.science)
    }

    /// The met rival major whose Science a turn is at least `margin` times
    /// every other met major's, ours included, with its reading and the
    /// runner-up's. `None` when we lead, or nobody leads by the margin.
    pub(crate) fn science_leader_by(
        g: &Game,
        pid: usize,
        margin: f64,
    ) -> Option<(usize, f64, f64)> {
        let mut readings: Vec<(f64, usize)> = g
            .players
            .iter()
            .filter(|other| {
                other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && (other.id == pid || g.has_met(pid, other.id))
            })
            .map(|other| (Self::seat_science_per_turn(g, other.id), other.id))
            .collect();
        readings.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let (best, leader) = *readings.first()?;
        let runner_up = readings.get(1).map_or(0.0, |(science, _)| *science);
        (leader != pid && best > 0.0 && best >= margin * runner_up.max(0.0))
            .then_some((leader, best, runner_up))
    }

    /// `science-leader-is-the-target`: with no major war running, the
    /// Science leader a Domination seat aims its campaign at. The leader is
    /// [`SCIENCE_LEADER_MARGIN`] ahead of every other met major -- or, if it
    /// is already the plan's rival, simply ahead, so a reading that dips
    /// under the margin does not bounce the army. It must be legal to
    /// target, pass the war-policy feasibility, hold a city within the
    /// declaration range, be reachable by land, and stand under our
    /// military [`DECLARATION_EDGE_RATIO`] times over its steady power.
    /// `None` with the gene off. See the module notes.
    pub(crate) fn science_leader_at_peace(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.science_leader_is_the_target
            || self.forced_target_player.is_some()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !Self::victory_strategy_enabled(g, GrandStrategy::Science)
            || g.turn < g.standard_duration(SCIENCE_LEADER_TURN)
            || !self.one_war_enemies(g, pid).is_empty()
        {
            return None;
        }
        let incumbent = self.plan.as_ref().and_then(|plan| plan.target_player);
        let (leader, _, _) =
            Self::science_leader_by(g, pid, SCIENCE_LEADER_MARGIN).or_else(|| {
                Self::science_leader_by(g, pid, 1.0)
                    .filter(|(leader, _, _)| Some(*leader) == incumbent)
            })?;
        (!g.is_at_war(pid, leader)
            && self.campaign_target_legal(g, pid, leader)
            && self.war_policy_target_feasible(g, pid, leader)
            && g.military_power(pid)
                >= DECLARATION_EDGE_RATIO * self.steady_rival_power(g, leader).max(1.0)
            && g.player_city_ids(leader)
                .iter()
                .any(|city| Self::city_within_declaration_range(g, pid, g.cities[city].pos))
            && self.rival_reachable_by_land(g, pid, leader))
        .then_some(leader)
    }
}

#[cfg(test)]
mod tests;
