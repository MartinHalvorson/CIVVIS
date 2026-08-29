//! `expansion-schedule`: when the empire is behind the pace that wins, open
//! the settler pipeline instead of walking one settler at a time.
//!
//! ## What the recorded games say
//!
//! `tools/civ6_run_report.py --aggregate` over `~/civvis-civ6-runs/control`,
//! 218 completed live runs (2026-08-25):
//!
//! | cities at turn 60 | runs | wins | rate |
//! |---|---:|---:|---:|
//! | 1–3 | 127 | **0** | 0% |
//! | 4 | 56 | 1 | 2% |
//! | 5 | 23 | 4 | 17% |
//! | 6 | 10 | 4 | 40% |
//! | **outside 4–6** | **128** | **0** | **0%** |
//!
//! **Every one of the nine recorded wins came from inside the band** — 3.7
//! expected if the two were independent, one-sided Fisher exact
//! *p* = 2.6 × 10⁻⁴. The modal run holds three cities at turn 60 (80 runs,
//! no wins), and the median loss loses the lead for good at **turn 61**.
//! Whatever else is wrong with these games, it has already happened by then.
//!
//! ## Why the empire is short, and what is not the reason
//!
//! It is not the target. `assess` already computes
//! `desired_cities = (floor + turn / cadence).min(map_capacity)` from a floor
//! of [`super::PRODUCTION_CITY_TARGET_FLOOR`] = 6, so the planner asks for
//! **seven cities by turn 60** and gets three.
//!
//! It is not the price either, and that is measured: `gene_census --gene
//! p_settler --games 96` left **97% of games outcome-identical with the
//! Settler scored at four times its shipped value** (the note on the lane
//! multiplier in `production_value`). A Settler is not blocked by losing a
//! ranking; it is blocked before the ranking is consulted.
//!
//! What blocks it is the pipeline. [`AdvancedAi::settler_in_flight_allowed`]
//! answers **1** for the ordinary empire — one walker at a time, built, then
//! marched, then founded, before the next one starts — and the two flags that
//! widen it, `land_grab` and `parallel_settlers`, are both `Kind::HostOnly`:
//! they are inert on a native board and cannot be screened at all.
//!
//! ## ⚠⚠⚠ What the live seat shows, and where the claim above is wrong
//!
//! The paragraph above says a Settler "is not blocked by losing a ranking; it
//! is blocked before the ranking is consulted". **On the live Civilization VI
//! seat that is measurably false**, and the difference decides the game.
//!
//! Run `civvis-20260829T084031Z`, King, 6p, this gene FORCED ON via
//! `~/.civvis-live-force-on`, capital trace (`producing` sampled every 6 turns,
//! with whether a Settler was in `buildable` that same turn):
//!
//! | turn | pop | producing | Settler buildable |
//! |---:|---:|---|---|
//! | 6 | 1 | — | no — the population floor, legitimately |
//! | 12 | 2 | `UNIT_SETTLER` | yes |
//! | 24 | 2 | `UNIT_ARCHER` | yes |
//! | 30 | 4 | `DISTRICT_CAMPUS` | yes |
//! | 36 | 4 | `BUILDING_WALLS` | yes |
//! | 42 | 5 | `UNIT_ARCHER` | yes |
//!
//! The pop drop at t24 is the first Settler completing. It then walked until
//! **t46** before founding, and across those 28 turns the capital had a Settler
//! buildable on every sampled turn and built Archer, Campus, Walls, Archer.
//! Opening production (t1–46) went **22% Settler, 78% other**, at one city,
//! while the corpus above says every recorded win came from 4–6 by turn 60.
//!
//! ⚠ At t30 the pipeline was NOT the blocker. Pace is 3, the empire held one
//! city and one walker, so [`AdvancedAi::expansion_pace_shortfall`] is 1 and
//! [`AdvancedAi::expansion_schedule_pipeline`] returns `Some(2)`: a second
//! Settler was allowed and the Campus won the ranking anyway. Widening the
//! pipeline cannot fix a city that does not want the Settler it may now build.
//!
//! ⚠ A second, narrower effect at t24: the shortfall counts a walker as a city
//! (`pace - (city_count + settlers)` = 2 - 2 = 0), so the gene declines to widen
//! while a Settler is in flight. That is deliberate — the doc below says
//! "counting the walkers already on their way" — but it assumes the walk
//! converts promptly, and this one took 28 turns. A slow or held walker credits
//! the empire with a city it does not have for as long as it walks.
//!
//! What it cost: 2 cities at turn 60, **55% of the leader by turn 53**, then a
//! flat ~3 score/turn against the leader's ~6, ending at 51.1% and abandoned at
//! turn 150 under the operator's rule — with **10 cities and zero settler
//! captures**, so neither expansion loss nor barbarians explain it. Three
//! completed games with the gene forced on gave cities-at-60 of 5, 2 and 2.
//!
//! ⚠ This is a MEASUREMENT, not a licence to re-price. `gene_census --gene
//! p_settler --games 96` left 97% of games outcome-identical at four times the
//! shipped Settler value, and the standing finding is that actuation repairs pay
//! where valuation tunes do not. Both results cannot be right about the same
//! seat: the census is headless and this is live, and headless and live are
//! different regimes. Resolving that needs a screen, not an edit.
//!
//! ## What the gene does
//!
//! It gives the opening a **pace** and lets the pipeline answer to it. The
//! band above is a schedule, not a total: [`AdvancedAi::expansion_pace`]
//! walks one city at a time to [`EXPANSION_BAND_FLOOR`] by the band turn
//! ([`EXPANSION_BAND_SHARE`] of the clock — turn 60 of the ladder's 250), so
//! it wants two cities a quarter of the way there and three a half. While the
//! empire is **behind that pace** the pipeline widens by the shortfall, to at
//! most [`EXPANSION_PIPELINE_CEILING`] walkers, and never past the seats the
//! empire is actually short — `desired_cities` stays the hard cap exactly as
//! it does for the land grab.
//!
//! Past the band turn the gene is inert: the corpus says nothing about pace
//! after turn 60, `desired_cities` climbs on its own from there, and
//! [`AdvancedAi::stock_expansion_deadline`] already runs the adaptive seat to
//! about turn 200. The claim here is about the opening, so the gene is only
//! about the opening.
//!
//! ⚠ It widens the pipeline and nothing else. It does not raise the Settler's
//! production value (measured null, above), does not touch `desired_cities`,
//! does not relax a site bar, and does not lower the engine's own population
//! floor — a city still has to reach population 2 to build a Settler at all,
//! which is its own constraint on 23.8% of seat-turns and is not this gene's
//! subject.

use super::AdvancedAi;
use crate::game::Game;

/// Where in the winning band the schedule aims by the band turn.
///
/// ⚠ THIS WAS 4 — THE FLOOR OF THE BAND — SO THE PACE AIMED AT THE WORST
/// OUTCOME THAT STILL WINS. Every recorded win came from FOUR TO SIX cities by
/// the band turn (9/9 inside, 0/128 outside, Fisher p=2.6e-4), and a target of
/// four asks the seat to arrive exactly on the bottom edge of that band. Any
/// slippage at all — one settler lost, one site denied, one war — lands it
/// outside, and the live cadence shows precisely that: the documented founding
/// turns for cities 2/3/4/5/6 are 37.0/71.0/89.5/118.7/150.2, so four cities
/// arrive around turn 90, not turn 60.
///
/// Five aims at the middle of the same measured band, so an ordinary setback
/// still lands inside it. It does not widen the band, invent a target outside
/// it, or touch `desired_cities`, which remains the hard cap on every branch
/// below.
///
/// The pace this feeds is `1 + (FLOOR - 1) * turn / band`, so raising it moves
/// the whole early curve rather than only the endpoint: at turns
/// 0/15/20/40/45/60 the pace goes 1/1/2/3/3/4 to 1/2/2/3/4/5.
pub const EXPANSION_BAND_FLOOR: usize = 5;

/// Where the band sits on the clock — turn 60 of the ladder's 250-turn Online
/// game, as a share, so a different turn limit scales with it.
pub const EXPANSION_BAND_SHARE: f64 = 0.24;

/// The band turn in standard turns, for a game played without a turn limit.
pub const EXPANSION_BAND_STANDARD: u32 = 120;

/// The most walkers the schedule will ever have in flight at once. Three is
/// the shortfall of an empire that reached the band turn with one city; a
/// fourth would be an empire that has a different problem.
pub const EXPANSION_PIPELINE_CEILING: usize = 3;

impl AdvancedAi {
    /// The turn the winning band is measured at.
    pub(super) fn expansion_band_turn(g: &Game) -> u32 {
        g.turn_limit()
            .map(|limit| ((limit as f64) * EXPANSION_BAND_SHARE) as u32)
            .unwrap_or_else(|| g.standard_duration(EXPANSION_BAND_STANDARD))
            .max(1)
    }

    /// How many cities the winning pace expects by now: one at the start,
    /// [`EXPANSION_BAND_FLOOR`] by the band turn, walked one at a time in
    /// between.
    pub(super) fn expansion_pace(g: &Game) -> usize {
        let band = Self::expansion_band_turn(g);
        if g.turn >= band {
            return EXPANSION_BAND_FLOOR;
        }
        1 + ((EXPANSION_BAND_FLOOR - 1) * g.turn as usize) / band as usize
    }

    /// How far behind the winning pace this empire is, counting the walkers
    /// already on their way. Zero while the gene is off, once the band turn
    /// has passed, and whenever the empire is on or ahead of pace.
    pub(super) fn expansion_pace_shortfall(
        &self,
        g: &Game,
        city_count: usize,
        settlers: usize,
    ) -> usize {
        if !self.expansion_schedule || g.turn > Self::expansion_band_turn(g) {
            return 0;
        }
        Self::expansion_pace(g).saturating_sub(city_count + settlers)
    }

    /// The pipeline width the schedule asks for, or `None` when it asks for
    /// nothing. Never wider than the seats the empire is short, so
    /// `desired_cities` remains the hard cap.
    pub(super) fn expansion_schedule_pipeline(
        &self,
        g: &Game,
        desired_cities: usize,
        city_count: usize,
        settlers: usize,
    ) -> Option<usize> {
        let shortfall = self.expansion_pace_shortfall(g, city_count, settlers);
        if shortfall == 0 || city_count + settlers >= desired_cities {
            return None;
        }
        Some(
            (1 + shortfall)
                .min(EXPANSION_PIPELINE_CEILING)
                .min(desired_cities - city_count),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::setup::GameSpeed;

    /// A 250-turn Online board, the shape the corpus was measured on.
    fn board() -> Game {
        let mut g = Game::new(2, 24, 16, 71, 250, 0);
        g.game_speed = GameSpeed::Online;
        g
    }

    #[test]
    fn off_by_default_and_toggles() {
        let ai = AdvancedAi::new();
        assert!(!ai.expansion_schedule, "an opt-in ships off");
        assert!(!AdvancedAi::legacy().expansion_schedule);
        let mut ai = AdvancedAi::new();
        ai.enable_expansion_schedule();
        assert!(ai.expansion_schedule);
        ai.disable_expansion_schedule();
        assert!(!ai.expansion_schedule);
    }

    #[test]
    fn the_band_turn_is_turn_sixty_of_the_ladders_clock() {
        let g = board();
        assert_eq!(AdvancedAi::expansion_band_turn(&g), 60);
    }

    #[test]
    fn the_pace_walks_one_city_at_a_time_to_the_band_floor() {
        let mut g = board();
        let seen: Vec<usize> = [0u32, 15, 20, 40, 45, 60, 100]
            .into_iter()
            .map(|turn| {
                g.turn = turn;
                AdvancedAi::expansion_pace(&g)
            })
            .collect();
        // Floor 5: the pace reaches two by t15 and five by the band turn.
        // With the old floor of four this read [1, 1, 2, 3, 3, 4, 4] — the
        // seat was asked for one city until t20 and four at the band.
        assert_eq!(seen, vec![1, 2, 2, 3, 4, 5, 5], "{seen:?}");
    }

    #[test]
    fn a_seat_on_pace_is_asked_for_nothing() {
        let mut g = board();
        g.turn = 40;
        let mut ai = AdvancedAi::new();
        ai.enable_expansion_schedule();
        // Pace at t40 is three; three cities is on pace, and two cities with a
        // walker already on its way is on pace too.
        assert_eq!(ai.expansion_pace_shortfall(&g, 3, 0), 0);
        assert_eq!(ai.expansion_pace_shortfall(&g, 2, 1), 0);
        assert_eq!(ai.expansion_schedule_pipeline(&g, 7, 3, 0), None);
    }

    #[test]
    fn a_seat_behind_the_pace_opens_the_pipeline_by_the_shortfall() {
        let mut g = board();
        g.turn = 40;
        let mut ai = AdvancedAi::new();
        ai.enable_expansion_schedule();
        // One city and no walker at t40 is two seats behind the pace of three.
        assert_eq!(ai.expansion_pace_shortfall(&g, 1, 0), 2);
        assert_eq!(ai.expansion_schedule_pipeline(&g, 7, 1, 0), Some(3));
        // One behind asks for two walkers, not three.
        assert_eq!(ai.expansion_schedule_pipeline(&g, 7, 2, 0), Some(2));
    }

    #[test]
    fn the_city_target_stays_the_hard_cap() {
        let mut g = board();
        g.turn = 40;
        let mut ai = AdvancedAi::new();
        ai.enable_expansion_schedule();
        // Two seats behind the pace, but the empire only wants two cities and
        // holds one: exactly one more walker, never the shortfall.
        assert_eq!(ai.expansion_schedule_pipeline(&g, 2, 1, 0), Some(1));
        // Already at the target: the schedule asks for nothing at all.
        assert_eq!(ai.expansion_schedule_pipeline(&g, 2, 2, 0), None);
    }

    #[test]
    fn the_schedule_is_the_openings_and_goes_quiet_after_the_band() {
        let mut g = board();
        let mut ai = AdvancedAi::new();
        ai.enable_expansion_schedule();
        g.turn = 60;
        // Four short of the band-turn pace of five; this read 3 while the
        // schedule aimed at the band's floor rather than its middle.
        assert_eq!(
            ai.expansion_pace_shortfall(&g, 1, 0),
            4,
            "the band turn still counts"
        );
        g.turn = 61;
        assert_eq!(ai.expansion_pace_shortfall(&g, 1, 0), 0, "past it, inert");
        assert_eq!(ai.expansion_schedule_pipeline(&g, 7, 1, 0), None);
    }

    #[test]
    fn the_gene_off_is_an_exact_no_op() {
        let mut g = board();
        g.turn = 40;
        let ai = AdvancedAi::new();
        assert_eq!(ai.expansion_pace_shortfall(&g, 1, 0), 0);
        assert_eq!(ai.expansion_schedule_pipeline(&g, 7, 1, 0), None);
    }
}
