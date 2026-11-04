//! `builder-prices-travel-in-turns`: a Builder prices a walk in the turns it
//! costs, not in hexes. Opt-in gene, off by default.
//!
//! ⚠⚠ 0.7 A HEX IS NOTHING AGAINST A JOB WORTH 5 TO 100. Every destination a
//! Builder weighs — the empire-wide sweep in `advanced_builder_step`,
//! `builder_jobs_ranked` and `more_useful_builder_job` — scores a job as
//! `value − 0.7 × wdist`. A city's per-city premiums (its production-
//! foundation shortfall, its housing and growth) swing a job's value by tens,
//! so a five-hex walk costs 3.5 against a gap of 20 or 40, and the Builder
//! crosses the empire for a job barely better than the one beside it.
//!
//! Live Immortal seats 2026-10-30 (30 seats, t≤120, -8b's census):
//! - 63% of a Builder's consecutive improvements sat near a DIFFERENT city,
//!   a median 6 turns and 5 hexes apart, against 2 turns and 2 hexes inside
//!   one city.
//! - 99% of improvements sat 1-2 tiles from a city and A-B-A oscillation was
//!   3%, so this is hopping between cities, not dithering.
//! - It took 7.6 Builder-turns per improvement, and the Builder was moving on
//!   82% of its turns. why.log carried 24.6 "Builder chooses more useful
//!   work" lines per game by t80, e.g. "adds 94.0 after travel, versus 6.6
//!   here".
//! - At t40 we had improved 5% of our land against the rivals' 26%.
//!
//! With the gene on, while the Builder has a charge to spare, a job scores its
//! value per Builder-turn: `value / turns`, where `turns` is the turn the work
//! is done on plus every whole turn of walking (`1 + ⌊wdist / moves⌋`; at two
//! moves a neighbour costs 1, two or three hexes cost 2, four or five cost 3).
//! Leaving work here for a job three turns off now needs that job to be worth
//! three times as much, not 2.1 more.
//!
//! ⚠ THE LAST CHARGE IS PRICED AS BEFORE. Ordering is the whole lever while
//! charges remain: the Builder does the near job first and still has charges
//! for the far one. On its last charge there is no "afterwards", so it goes to
//! the best job by value exactly as the untreated rule sends it, and a distant
//! luxury or a shortfall city's mine is never starved for a nearer, smaller
//! job.

use super::*;

/// How a Builder prices the walk to a job. Read once per decision with
/// [`AdvancedAi::builder_travel`], then applied to every candidate tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BuilderTravel {
    /// The Builder's full movement when the walk is priced in turns; `None`
    /// is the untreated `0.7` a hex.
    moves: Option<f64>,
}

/// The historical price of a hex of travel.
const HEX_PRICE: f64 = 0.7;

impl BuilderTravel {
    /// The untreated rule, `value − 0.7 × wdist`.
    pub(super) const LEGACY: Self = Self { moves: None };

    /// Price walks in turns at `moves` a turn.
    pub(super) fn in_turns(moves: f64) -> Self {
        Self {
            moves: Some(moves.max(1.0)),
        }
    }

    /// Whether this price is the gene's (in turns) rather than the untreated one.
    pub(super) fn priced_in_turns(self) -> bool {
        self.moves.is_some()
    }

    /// Turns from now until a job `distance` hexes away is done: the working
    /// turn plus every whole turn of walking.
    pub(super) fn turns(self, distance: i32) -> f64 {
        let moves = self.moves.unwrap_or(1.0);
        1.0 + (f64::from(distance.max(0)) / moves).floor()
    }

    /// The score of a job worth `value`, `distance` hexes away.
    ///
    /// ⚠ The untreated arm must stay `value - distance as f64 * 0.7` exactly:
    /// it is what the three call sites computed inline, and an opt-in gene
    /// that is off changes no decision.
    pub(super) fn score(self, value: f64, distance: i32) -> f64 {
        if self.moves.is_none() {
            return value - f64::from(distance) * HEX_PRICE;
        }
        let turns = self.turns(distance);
        // A non-positive job must still look worse further away: dividing it
        // would pull it toward zero, i.e. toward "better".
        if value > 0.0 {
            value / turns
        } else {
            value * turns
        }
    }
}

impl AdvancedAi {
    /// How Builder `uid` prices its walks this turn. Untreated unless the gene
    /// is on and the Builder holds more than one charge.
    pub(super) fn builder_travel(&self, g: &Game, uid: u32) -> BuilderTravel {
        if !self.builder_prices_travel_in_turns {
            return BuilderTravel::LEGACY;
        }
        let Some(unit) = g.units.get(&uid) else {
            return BuilderTravel::LEGACY;
        };
        if unit.charges <= 1 {
            return BuilderTravel::LEGACY;
        }
        BuilderTravel::in_turns(g.unit_max_moves(uid))
    }
}

#[cfg(test)]
mod tests;
