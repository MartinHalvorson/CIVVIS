//! `rapid-city-expansion-2`: reach the opening band that wins, then let the
//! ordinary city economy breathe.
//!
//! Version one is the worst row in the current standard-screen ranking:
//! **-4.65 percentage points of wins** and roughly **+16%** compute/time. Its
//! implementation asks for as many as fifteen cities immediately, opens three
//! Settler slots from the first city and widens to six, replaces non-empty
//! peaceful queues, takes the closest legal site regardless of the travel
//! premium, and converts an exhausted frontier into a Conquest plan.
//!
//! The same screens resolve several narrower expansion pieces positively:
//! `wide-map-capacity`, `capital-settler-after-completion`,
//! `expansion-schedule`, and `city-target-meets-the-map`. Version two composes
//! their shape without switching those independent genes on: the ordinary
//! land-aware target remains authoritative; the opening target cannot lag the
//! measured five-city pace; a pipeline opens only while the seat is behind
//! that pace; only an empty capital queue is reserved; a new Settler needs an
//! acceptable unclaimed seat; and its walker must clear the travel-adjusted
//! site floor. The founding-pantheon override, closest-site override, queue
//! replacement, ever-widening pipeline, and automatic war are deliberately
//! version-one-only.
//!
//! This module owns the two pure quantities shared by `AdvancedAi` and the
//! delegated `BasicAi` governor, so the strategic and production halves cannot
//! drift to different opening schedules.

use super::expansion_schedule::{
    EXPANSION_BAND_FLOOR, EXPANSION_BAND_SHARE, EXPANSION_BAND_STANDARD, EXPANSION_PIPELINE_CEILING,
};
use crate::game::Game;

/// Turn at which the measured 4–6 city band is read.
fn band_turn(g: &Game) -> u32 {
    g.turn_limit()
        .map(|limit| ((limit as f64) * EXPANSION_BAND_SHARE) as u32)
        .unwrap_or_else(|| g.standard_duration(EXPANSION_BAND_STANDARD))
        .max(1)
}

/// City count the selective rewrite expects at this point in the opening.
fn pace(g: &Game) -> usize {
    let band = band_turn(g);
    if g.turn >= band {
        return EXPANSION_BAND_FLOOR;
    }
    1 + ((EXPANSION_BAND_FLOOR - 1) * g.turn as usize) / band as usize
}

/// Raise the ordinary land-aware target only when it trails the measured
/// opening pace. The ordinary ceiling remains a hard cap.
pub(super) fn city_target(g: &Game, ordinary: usize, ceiling: usize) -> usize {
    ordinary.max(pace(g)).min(ceiling)
}

/// Pipeline width while the empire is behind the measured opening pace.
///
/// `None` means the rewrite is no longer asking for a widened pipeline; the
/// caller falls through to every ordinary gate. Unlike version one's
/// ever-widening 3..6 walkers, this is at most the measured schedule's three
/// and becomes inert after the band turn.
pub(in crate::ai) fn pipeline_width(
    g: &Game,
    desired_cities: usize,
    city_count: usize,
    settlers: usize,
) -> Option<usize> {
    if g.turn > band_turn(g) || city_count + settlers >= desired_cities {
        return None;
    }
    let shortfall = pace(g).saturating_sub(city_count + settlers);
    if shortfall == 0 {
        return None;
    }
    Some(
        (1 + shortfall)
            .min(EXPANSION_PIPELINE_CEILING)
            .min(desired_cities - city_count),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::GameSpeed;

    fn board() -> Game {
        let mut g = Game::new(2, 24, 16, 72, 250, 0);
        g.game_speed = GameSpeed::Online;
        g
    }

    #[test]
    fn target_reaches_the_middle_of_the_winning_band_without_becoming_fifteen() {
        let mut g = board();
        g.turn = 60;
        assert_eq!(city_target(&g, 3, 9), 5);
        assert_eq!(city_target(&g, 6, 9), 6, "the ordinary target may lead");
        assert_eq!(city_target(&g, 3, 4), 4, "the map ceiling remains hard");
    }

    #[test]
    fn pipeline_is_bounded_to_the_opening_and_the_city_target() {
        let mut g = board();
        g.turn = 40;
        assert_eq!(pipeline_width(&g, 9, 1, 0), Some(3));
        assert_eq!(pipeline_width(&g, 2, 1, 0), Some(1));
        assert_eq!(pipeline_width(&g, 9, 2, 1), None, "walkers count");
        g.turn = 61;
        assert_eq!(pipeline_width(&g, 9, 1, 0), None, "past the band");
    }
}
