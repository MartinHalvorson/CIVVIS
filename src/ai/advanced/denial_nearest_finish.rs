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

/// A late culture leader whose Tourism runs this many times the next
/// civilization's compounds faster than its visitor curve shows: Flight,
/// Computers and the Heritage Organizations multiply it, and the visitors
/// then double every seven or eight turns. Live King civvis-20261003T090618Z
/// (game 29): Canada at turn 146 had 146 Tourism against 66, and 18 visitors
/// against a bar of 81, a 60-turn projection; it won at 169. Live King
/// civvis-20261003T103619Z (game 32): the Maori at turn 150 had 118 against
/// about 65, and 16 visitors against 71; they won at 178 (diagnosed by -60).
pub(super) const CULTURE_SURGE_TOURISM_RATIO: f64 = 1.5;
/// The share of the bar a surging leader's visitors must already hold.
pub(super) const CULTURE_SURGE_BAR_SHARE: f64 = 0.2;
/// Visitors at this share of the bar read as a surge whatever the Tourism
/// ranking: two culture civilizations can run within the ratio of each
/// other. Live King civvis-20261003T115745Z (game 35): Hungary led Spain on
/// Tourism by only 1.1-1.5 times, held 23 visitors against a bar of 99 at
/// turn 180 and 30 against 113 at 190, and won on Culture at 200 while the
/// army fought Spain (diagnosed by -60).
pub(super) const CULTURE_SURGE_SHARE_ALONE: f64 = 0.25;
/// Standard turns before which no surge is read: early Tourism leads are
/// small numbers.
pub(super) const CULTURE_SURGE_TURN: u32 = 200;
/// The reading a surging leader's clock takes when its curve projects no
/// nearer finish: past the urgent bar of the Domination counter.
pub(super) const CULTURE_SURGE_PROGRESS: i32 = 85;

/// Standard turns of foreign-tourist and bar readings the projection reads.
pub(super) const CULTURE_CURVE_WINDOW: u32 = 20;

/// The fewest standard turns between the first and last reading before a
/// curve is projected: a few turns of tourism is noise.
pub(super) const CULTURE_CURVE_MIN_SPAN: u32 = 8;

/// `culture-finish-at-the-observed-bar`: the share of the exported bar at
/// which a culture race is read as finished. Just under the median (94%) of
/// the 22 October culture losses' last readings.
pub(crate) const CULTURE_OBSERVED_BAR: f64 = 0.92;

/// `culture-reads-the-engine-clock`: the culture pressure a rival keeps
/// when the host reports no path to its Culture Victory, below the urgency
/// bars.
pub(crate) const ENGINE_NO_PATH_CULTURE_CAP: i32 = 50;

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
    /// `culture-reads-the-engine-clock`: the host's own turns to `rival`'s
    /// Culture Victory (`GetTurnsUntilVictory`, the World Rankings screen's
    /// clock): `Some(Some(turns))` on a path, `Some(None)` when the engine
    /// reports none, `None` without the gene or the reading. Our tourist ratio
    /// misreads the race: it compares visitors with the largest staycation,
    /// and staycations fall as the leader draws their tourists away. Every one
    /// of the 22 October culture losses fired below 100% of it, and live King
    /// civvis-20261005T065548Z (game 107) read France at 59-75% against
    /// Babylon's 50-68% over turns 201-203, turned the counter on France at
    /// 202, and Babylon won on Culture at 206.
    pub(super) fn engine_culture_clock(&self, g: &Game, rival: usize) -> Option<Option<f64>> {
        if !self.culture_reads_the_engine_clock {
            return None;
        }
        let turns = g.culture_turns_to_victory(rival)?;
        Some((turns >= 0.0).then_some(turns))
    }

    /// `culture-reads-the-engine-clock`: `raw`, the tourist-ratio culture
    /// pressure, replaced by the engine's clock when it reports one -- 100 at
    /// a finish now, 75 at [`DENIAL_FINISH_HORIZON`] turns, the reading
    /// `nearest_finish_culture_clock` gives a projection -- and held under
    /// [`ENGINE_NO_PATH_CULTURE_CAP`] when the engine reports no path.
    pub(crate) fn engine_culture_pressure(&self, g: &Game, rival: usize, raw: i32) -> i32 {
        match self.engine_culture_clock(g, rival) {
            None => raw,
            Some(None) => raw.min(ENGINE_NO_PATH_CULTURE_CAP),
            Some(Some(turns)) => (100.0 - turns * 100.0 / (4.0 * DENIAL_FINISH_HORIZON))
                .round()
                .clamp(0.0, 100.0) as i32,
        }
    }

    pub(super) fn projected_culture_finish(&self, g: &Game, rival: usize) -> Option<f64> {
        self.projected_culture_finish_at(g, rival, 1.0)
    }

    /// `culture-finish-at-the-observed-bar`: the finish a clock weighing a
    /// march against a culture race should read — at [`CULTURE_OBSERVED_BAR`]
    /// of the exported bar under the gene, at the bar otherwise. Every one of
    /// the 22 culture losses of October 4-5 fired with the winner's last
    /// reading of foreign tourists below the largest other domestic count:
    /// median 94%, 79% to 99% (civvis-20261005T061801Z: Canada 183 against
    /// our 194; T063500Z: Scythia 179 against Scotland's 188). The urgency
    /// gates keep the exported bar.
    pub(super) fn observed_culture_finish(&self, g: &Game, rival: usize) -> Option<f64> {
        let scale = if self.culture_finish_at_the_observed_bar {
            CULTURE_OBSERVED_BAR
        } else {
            1.0
        };
        self.projected_culture_finish_at(g, rival, scale)
    }

    /// Turns until `rival`'s foreign tourists pass `scale` times the bar,
    /// as [`Self::projected_culture_finish`] reads it at 1.0.
    fn projected_culture_finish_at(&self, g: &Game, rival: usize, scale: f64) -> Option<f64> {
        // See `engine_culture_clock`: the host's own turns, at any scale.
        if let Some(clock) = self.engine_culture_clock(g, rival) {
            return clock;
        }
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
        let target = bar.max(1) as f64 * scale;
        if foreign as f64 >= target {
            return Some(0.0);
        }
        let growth = ((foreign as f64 / first_foreign as f64).ln()
            - (bar.max(1) as f64 / first_bar.max(1) as f64).ln())
            / f64::from(span);
        (growth > 0.0).then(|| (target / foreign as f64).ln() / growth)
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
        let projected = self
            .projected_culture_finish(g, rival)
            .filter(|turns| *turns <= DENIAL_FINISH_HORIZON)
            .map(|turns| {
                (100.0 - turns * 100.0 / (4.0 * DENIAL_FINISH_HORIZON))
                    .round()
                    .clamp(0.0, 100.0) as i32
            });
        let surge = self
            .culture_surge(g, rival)
            .then_some(CULTURE_SURGE_PROGRESS);
        projected.max(surge).map(|progress| VictoryFocus {
            strategy: GrandStrategy::Culture,
            progress,
        })
    }

    /// Whether `rival` surges in the culture race after
    /// [`CULTURE_SURGE_TURN`]: visitors at [`CULTURE_SURGE_SHARE_ALONE`] of
    /// the bar it must pass, or at [`CULTURE_SURGE_BAR_SHARE`] with Tourism
    /// [`CULTURE_SURGE_TOURISM_RATIO`] times every other living major's.
    pub(super) fn culture_surge(&self, g: &Game, rival: usize) -> bool {
        if g.turn < g.standard_duration(CULTURE_SURGE_TURN) {
            return false;
        }
        let living: Vec<usize> = g
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        let next = living
            .iter()
            .filter(|other| **other != rival)
            .map(|other| g.tourism_per_turn(*other))
            .fold(0.0_f64, f64::max);
        let bar = living
            .iter()
            .filter(|other| **other != rival)
            .map(|other| g.domestic_tourists(*other))
            .max()
            .unwrap_or(1)
            .max(1);
        let share = g.foreign_tourists(rival) as f64 / bar as f64;
        share >= CULTURE_SURGE_SHARE_ALONE
            || (g.tourism_per_turn(rival) >= CULTURE_SURGE_TOURISM_RATIO * next.max(1.0)
                && share >= CULTURE_SURGE_BAR_SHARE)
    }

    /// Whether `rival`'s culture race is projected to finish within
    /// [`DENIAL_URGENT_FINISH`] game turns. See `urgent_victory_threat`.
    pub(super) fn culture_finish_is_urgent(&self, g: &Game, rival: usize) -> bool {
        self.deny_leaders
            && self.nearest_finish_culture_clock(g, rival).is_some()
            && (self
                .projected_culture_finish(g, rival)
                .is_some_and(|turns| turns <= DENIAL_URGENT_FINISH)
                || self.culture_surge(g, rival))
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

    /// Game 29's Canada at turn 146: 146 Tourism against 66 and 54, 18
    /// visitors against a bar of 81. The visitor curve projects no near
    /// finish, but the surge makes it an urgent clock.
    #[test]
    fn a_late_tourism_surge_is_an_urgent_clock() {
        let surge_case = |turn: u32, tourism: f64| {
            let mut g = Game::new_full(3, 24, 16, 9303, 300, 0, false);
            g.turn = turn;
            tourists(&mut g, [0, 18, 0], [26, 142, 81]);
            let observed = std::sync::Arc::make_mut(&mut g.observed_tourism_per_turn);
            observed.insert(0, 54.0);
            observed.insert(1, tourism);
            observed.insert(2, 66.0);
            let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
            ai.enable_denial_nearest_finish();
            ai.deny_leaders = true;
            (g, ai)
        };
        let late = |g: &Game| g.standard_duration(super::CULTURE_SURGE_TURN);
        let (mut g, ai) = surge_case(0, 146.0);
        g.turn = late(&g);
        assert_eq!(ai.projected_culture_finish(&g, 1), None, "no curve yet");
        assert!(ai.culture_surge(&g, 1));
        let clock = ai
            .nearest_finish_culture_clock(&g, 1)
            .expect("a surge clock");
        assert!(ai.domination_counter_pressure(&g, clock));
        assert!(ai.culture_finish_is_urgent(&g, 1));
        assert!(ai.urgent_victory_threat(&g, 1));

        // Too early, or a lead under the ratio, is no surge.
        let (mut g, ai) = surge_case(0, 146.0);
        g.turn = late(&g) - 1;
        assert!(!ai.culture_surge(&g, 1));
        let (mut g, ai) = surge_case(0, 90.0);
        g.turn = late(&g);
        assert!(!ai.culture_surge(&g, 1));
        assert!(ai.nearest_finish_culture_clock(&g, 1).is_none());

        // Game 35's Hungary: a lead under the ratio, but visitors already a
        // quarter of the bar.
        let (mut g, ai) = surge_case(0, 147.0);
        g.turn = late(&g);
        tourists(&mut g, [0, 27, 0], [26, 142, 100]);
        std::sync::Arc::make_mut(&mut g.observed_tourism_per_turn).insert(2, 103.0);
        assert!(ai.culture_surge(&g, 1), "a quarter of the bar is a surge");
    }
}
