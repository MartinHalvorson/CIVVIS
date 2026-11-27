//! `rival-science-run-opens-the-spy-civics`: a seat off the Science aim that
//! watches a rival's science run takes the civics that open Spy slots
//! (Diplomatic Service, then Nationalism) ahead of the government ladder.
//!
//! The space-race denial (`science-denial-trains-spies`,
//! `denial-spies-fill-every-slot-2`, `seen-pad-starts-the-denial`) can only
//! train the Spies our capacity allows, and capacity comes from civics.
//! `spies-steal-the-lead` and `spy-civics-come-first` bring those civics
//! forward, but only under a Science aim. Tournament 8 (Lv 8 Deity, Diplo
//! off), five Religion seats lost to a rival's Science Victory with launches
//! 0/4 against 4/4, and every one adopted Diplomatic Service late as a step
//! toward the government ladder ("the most valuable step toward class
//! struggle, which religion needs"), then trained its first denial Spy
//! within three to seven turns of it:
//!
//! | game | rival lead at t100 (reviewer) | Diplomatic Service | first Spy |
//! |---|---|---|---|
//! | `civvis-t8-20261126T111900Z` | 149.5 vs 58 Science | t151 | t158 |
//! | `civvis-t8-20261126T123528Z` | — | t126 | t133 |
//! | `civvis-t8-20261126T162529Z` | 38 vs 21 techs | t115 | t121 |
//! | `civvis-t8-20261127T034217Z` | 692 vs 96 Science at the end | t138 | t142 |
//! | `civvis-t8-20261127T041052Z` | 188 vs 68.5 Science | t127 | t130 |
//!
//! By then the rival had landed one or two space projects; the denial held
//! one or two Spies, each starting at level 0 and spending turns on Gain
//! Sources before its first Disrupt Rocketry.
//!
//! On, off a Science aim, while a met living rival major makes at least
//! [`RIVAL_SCIENCE_RUN_RATIO`] times our Science a turn and at least
//! [`RIVAL_SCIENCE_RUN_FLOOR`], the civic slot's forced goal is Diplomatic
//! Service, then Nationalism (one Spy slot each), after the Religion seat's
//! Theology/Reformed Church goal and ahead of the government ladder. The
//! Spies themselves are still trained and sent by the denial genes.
//! `Kind::OptIn`; off, the civic order is unchanged.

use super::*;

/// A met rival making at least this many times our Science a turn is
/// running away with the science race. Tournament 8's losses read 2.6-2.7
/// at turn 100.
pub(crate) const RIVAL_SCIENCE_RUN_RATIO: f64 = 1.5;

/// The rival's Science a turn under which no run is read: early-game
/// readings (8 against 5) are noise, not a space race.
pub(crate) const RIVAL_SCIENCE_RUN_FLOOR: f64 = 60.0;

/// The Spy-slot civics in the order the goal takes them.
pub(crate) const RIVAL_RUN_SPY_CIVICS: [&str; 2] = ["diplomatic_service", "nationalism"];

impl AdvancedAi {
    /// The met living rival major running the science race: the most
    /// Science a turn, at least [`RIVAL_SCIENCE_RUN_RATIO`] times ours and
    /// [`RIVAL_SCIENCE_RUN_FLOOR`]. `None` when nobody is.
    pub(crate) fn rival_science_run(g: &Game, pid: usize) -> Option<usize> {
        let ours = Self::seat_science_per_turn(g, pid).max(0.0);
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !g.same_team(pid, rival.id)
                    && g.has_met(pid, rival.id)
            })
            .map(|rival| (Self::seat_science_per_turn(g, rival.id), rival.id))
            .filter(|(science, _)| {
                *science >= RIVAL_SCIENCE_RUN_FLOOR && *science >= ours * RIVAL_SCIENCE_RUN_RATIO
            })
            .max_by(|(left, left_id), (right, right_id)| {
                left.total_cmp(right).then(right_id.cmp(left_id))
            })
            .map(|(_, rival)| rival)
    }

    /// The civic slot's forced goal under the gene: the first Spy-slot
    /// civic we lack while a rival runs the science race off a Science aim.
    /// `None` with the gene off.
    pub(super) fn rival_science_run_spy_civic_goal(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<&'static str> {
        if !self.rival_science_run_opens_the_spy_civics
            || plan.strategy == GrandStrategy::Science
            || self.active_victory_target(g) == Some(VictoryTarget::Science)
        {
            return None;
        }
        let held = &g.players[pid].civics;
        let goal = RIVAL_RUN_SPY_CIVICS.into_iter().find(|civic| {
            g.rules.civics.contains_key(*civic) && !held.contains(&Name::new(civic))
        })?;
        Self::rival_science_run(g, pid).map(|_| goal)
    }
}

#[cfg(test)]
mod tests;
