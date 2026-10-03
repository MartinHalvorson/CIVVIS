//! `denial-nearest-finish`: a Domination army counters the culture race by
//! when it will finish, not by where it stands today.
//!
//! A culture race grows geometrically: tourism compounds through wonders,
//! great works and their theming, so a rival's foreign tourists can double
//! in twenty turns while the largest domestic count it must pass barely
//! moves. The Domination counter read that race as a percentage against a
//! 50 bar and ranked it against the other clocks by that percentage.
//!
//! Live King civvis-20261003T060034Z lost on Culture to Sweden at turn 205.
//! Sweden's foreign tourists went 15, 38, 93 at turns 165, 185 and 205
//! against a bar of 71, 94 and 96. At turn 185 it read 40 percent, under the
//! bar, while the army fought Canada, the science leader, which had not
//! launched anything. Projected along its curve, Sweden crossed in about 28
//! turns from 185. Live King civvis-20261003T052455Z lost to Spain's culture
//! the same way at turn 182.

use super::{AdvancedAi, GrandStrategy, VictoryFocus, VictoryTarget};
use crate::game::Game;
use std::collections::BTreeMap;

/// A culture race projected to finish within this many turns is a clock the
/// Domination army answers: time to reach and take a city. Game turns, not
/// standard turns: an army marches the same tiles a turn at any speed.
pub(super) const DENIAL_FINISH_HORIZON: f64 = 30.0;

/// A culture race projected to finish within this many game turns is an
/// urgent threat: the one-war gate opens a second front against it.
pub(super) const DENIAL_URGENT_FINISH: f64 = 20.0;

/// Standard turns of foreign-tourist and bar readings the projection reads.
pub(super) const CULTURE_CURVE_WINDOW: u32 = 20;

/// The fewest standard turns between the first and last reading before a
/// curve is projected: a few turns of tourism is noise.
pub(super) const CULTURE_CURVE_MIN_SPAN: u32 = 8;

impl AdvancedAi {
    /// One reading per living rival per turn: its foreign tourists and the
    /// largest domestic count among the others, which it must pass.
    pub(super) fn record_culture_curves(&mut self, g: &Game, pid: usize) {
        if !self.denial_nearest_finish {
            return;
        }
        let living: Vec<usize> = g
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        let domestic: BTreeMap<usize, i64> = living
            .iter()
            .map(|player| (*player, g.domestic_tourists(*player)))
            .collect();
        let horizon = g
            .turn
            .saturating_sub(g.standard_duration(CULTURE_CURVE_WINDOW));
        for rival in living.iter().copied().filter(|rival| *rival != pid) {
            let bar = living
                .iter()
                .filter(|other| **other != rival)
                .map(|other| domestic[other])
                .max()
                .unwrap_or(1)
                .max(1);
            let history = self.culture_curves.entry(rival).or_default();
            history.retain(|(turn, _, _)| *turn >= horizon && *turn != g.turn);
            history.push((g.turn, g.foreign_tourists(rival), bar));
        }
        self.culture_curves.retain(|rival, history| {
            living.contains(rival) && history.last().is_some_and(|(turn, _, _)| *turn >= horizon)
        });
    }

    /// Turns until `rival`'s foreign tourists pass the bar, projecting both
    /// along their geometric growth over the recorded window. `None` without
    /// [`CULTURE_CURVE_MIN_SPAN`] standard turns of readings, without
    /// visitors to project, or while the bar keeps pace.
    pub(super) fn projected_culture_finish(&self, g: &Game, rival: usize) -> Option<f64> {
        let history = self.culture_curves.get(&rival)?;
        let (first_turn, first_foreign, first_bar) = *history.first()?;
        let (last_turn, foreign, bar) = *history.last()?;
        let span = last_turn.saturating_sub(first_turn);
        if span < g.standard_duration(CULTURE_CURVE_MIN_SPAN).max(2)
            || first_foreign <= 0
            || foreign <= 0
        {
            return None;
        }
        if foreign >= bar {
            return Some(0.0);
        }
        let growth = ((foreign as f64 / first_foreign as f64).ln()
            - (bar.max(1) as f64 / first_bar.max(1) as f64).ln())
            / f64::from(span);
        (growth > 0.0).then(|| (bar as f64 / foreign as f64).ln() / growth)
    }

    /// The culture clock a Domination army answers when `rival`'s race is
    /// projected to finish within [`DENIAL_FINISH_HORIZON`]. It reads 100 at
    /// a finish now, falling to 75 at the horizon, so a nearer finish
    /// outranks a higher reading.
    pub(super) fn nearest_finish_culture_clock(
        &self,
        g: &Game,
        rival: usize,
    ) -> Option<VictoryFocus> {
        if !self.denial_nearest_finish
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !Self::victory_strategy_enabled(g, GrandStrategy::Culture)
        {
            return None;
        }
        let turns = self.projected_culture_finish(g, rival)?;
        (turns <= DENIAL_FINISH_HORIZON).then(|| VictoryFocus {
            strategy: GrandStrategy::Culture,
            progress: (100.0 - turns * 100.0 / (4.0 * DENIAL_FINISH_HORIZON))
                .round()
                .clamp(0.0, 100.0) as i32,
        })
    }

    /// Whether `rival`'s culture race is projected to finish within
    /// [`DENIAL_URGENT_FINISH`] game turns. See `urgent_victory_threat`.
    pub(super) fn culture_finish_is_urgent(&self, g: &Game, rival: usize) -> bool {
        self.deny_leaders
            && self.nearest_finish_culture_clock(g, rival).is_some()
            && self
                .projected_culture_finish(g, rival)
                .is_some_and(|turns| turns <= DENIAL_URGENT_FINISH)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AdvancedAi, GrandStrategy, VictoryTarget};
    use super::{CULTURE_CURVE_MIN_SPAN, DENIAL_FINISH_HORIZON, DENIAL_URGENT_FINISH};
    use crate::game::Game;

    fn tourists(g: &mut Game, foreign: [usize; 3], domestic: [usize; 3]) {
        let stats = std::sync::Arc::make_mut(&mut g.observed_public_empire_stats);
        for pid in 0..3 {
            stats.insert(
                pid,
                crate::game::ObservedPublicEmpireStats {
                    domestic_tourists: Some(domestic[pid]),
                    foreign_tourists: Some(foreign[pid]),
                    ..Default::default()
                },
            );
        }
    }

    /// Sweden's live curve (turns 165 and 185 of civvis-20261003T060034Z):
    /// 15 then 38 visitors against a bar of 71 then 94. Its 40 percent
    /// reading is under the 50 bar, yet it crosses in about 28 turns.
    #[test]
    fn a_compounding_culture_race_is_projected_to_its_finish() {
        let mut g = Game::new_full(3, 24, 16, 9301, 300, 0, false);
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_denial_nearest_finish();
        g.turn = 165;
        tourists(&mut g, [0, 15, 9], [71, 93, 71]);
        ai.record_culture_curves(&g, 0);
        assert_eq!(ai.projected_culture_finish(&g, 1), None, "one reading");
        g.turn = 185;
        tourists(&mut g, [0, 38, 27], [94, 154, 94]);
        ai.record_culture_curves(&g, 0);
        let turns = ai.projected_culture_finish(&g, 1).expect("a projection");
        assert!((20.0..35.0).contains(&turns), "{turns}");
        assert!(ai.rival_culture_pressures(&g)[&1] < ai.culture_threat_pressure());
        let horizon = DENIAL_FINISH_HORIZON;
        assert!(turns <= horizon, "{turns} against {horizon}");
        let clock = ai
            .nearest_finish_culture_clock(&g, 1)
            .expect("inside the horizon");
        assert_eq!(clock.strategy, GrandStrategy::Culture);
        assert!(ai.domination_counter_pressure(&g, clock));
        // A clock to answer, not yet one that opens a second front.
        assert!(turns > DENIAL_URGENT_FINISH);
        assert!(!ai.culture_finish_is_urgent(&g, 1));
        assert!(!ai.urgent_victory_threat(&g, 1));
        // A slower race projects beyond the horizon and is not a clock.
        assert!(ai
            .projected_culture_finish(&g, 2)
            .is_some_and(|turns| turns > horizon));
        assert!(ai.nearest_finish_culture_clock(&g, 2).is_none());
    }

    #[test]
    fn a_near_finish_outranks_a_higher_reading_and_the_gene_is_opt_in() {
        let mut g = Game::new_full(3, 24, 16, 9302, 300, 0, false);
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        let span = g.standard_duration(CULTURE_CURVE_MIN_SPAN);
        g.turn = 150;
        tourists(&mut g, [0, 20, 0], [100, 50, 100]);
        ai.record_culture_curves(&g, 0);
        g.turn = 150 + span;
        tourists(&mut g, [0, 50, 0], [100, 50, 100]);
        ai.record_culture_curves(&g, 0);
        assert!(ai.culture_curves.is_empty(), "off records nothing");
        assert!(ai.nearest_finish_culture_clock(&g, 1).is_none());

        ai.enable_denial_nearest_finish();
        g.turn = 150;
        tourists(&mut g, [0, 20, 0], [100, 50, 100]);
        ai.record_culture_curves(&g, 0);
        g.turn = 150 + span;
        tourists(&mut g, [0, 50, 0], [100, 50, 100]);
        ai.record_culture_curves(&g, 0);
        let turns = ai.projected_culture_finish(&g, 1).expect("a projection");
        assert!(turns < span as f64, "{turns}");
        let clock = ai.nearest_finish_culture_clock(&g, 1).expect("near");
        assert!(clock.progress > 78, "{}", clock.progress);
        assert!(ai.culture_finish_is_urgent(&g, 1));
        assert!(ai.urgent_victory_threat(&g, 1), "a near finish is urgent");
        assert_eq!(ai.rival_culture_pressures(&g)[&1], 50);
    }
}
