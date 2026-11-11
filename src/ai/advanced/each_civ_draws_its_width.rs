//! `each-civ-draws-its-width`: every major draws, once per game, how wide it
//! builds — a city count past the shared ten — and its plan's city target
//! walks up [`CITY_WIDTH_STEP`] cities ahead of what it holds until it gets
//! there.
//!
//! ## Why
//!
//! Operator, 2026-11-10: *"we shouldn't have civs always build out only 10
//! cities. there are benefits in civ 6 to going wider. we should sometimes
//! (often?) build more. maybe 11, 12, 15, 20? we can try playing around more
//! to find the right number"*
//!
//! Operator, 2026-11-11, watching civvis.ai/test: *"everything there is still
//! building 10 cities. Please try build more as an experiment. [...] Play
//! around with the exact right amount more. Every civ shouldn't just have 10
//! though"*
//!
//! [`SHARED_CITY_TARGET_FLOOR`] is the ceiling in practice: under
//! `rapid-city-expansion-2` the era cadence does not pass it until the last
//! fifty turns, so on the site's own world nearly every major sits on exactly
//! ten cities from turn 150 to the end.
//!
//! ## The measurement (site world, 2026-11-10)
//!
//! The stock civvis.ai world (`stock_opening_params`: 4 majors, Tennis Ball
//! planet, Prince, Online), every major on the deployment genome. Chairs
//! {0,1} played the width and {2,3} the shared ten, then the same seed again
//! with the chairs swapped; 40 seeds per width, deltas are treated minus
//! untreated majors of the same world, averaged over the pair:
//!
//! | width | cities t150 | science t150 | production t150 | culture t150 | wins of 80 |
//! |---|---|---|---|---|---|
//! | 11 | +0.9 | +9.9% (se 3.4) | +11.4% | +8.8% | 40 |
//! | 12 | +1.8 | +7.7% (se 1.5) | +11.6% | +6.7% | 40 |
//! | 13 | +2.7 | +5.4% (se 1.7) | +14.2% | +4.3% | 38 |
//! | 14 | +3.7 | +6.8% (se 1.9) | +17.5% | +1.4% | 39 |
//! | 15 | +4.3 | +3.6% (se 1.5) | +14.1% | +3.6% | 37 |
//! | 20 | +5.5 | 0.0% (se 1.5) | +14.7% | +1.5% | 36 |
//!
//! 478 of the 480 worlds ended in a Science victory, at a median turn 191.
//! Every width past ten out-produced ten. Science and Culture gain most at
//! eleven to fourteen and fade from there; twenty kept the Production, gave
//! the Science back and cost wins (-0.10 a pair, se 0.05). Cities at turn 100
//! are +0.8 in every arm from thirteen up: one Settler at a time, the count
//! past fourteen arrives after turn 150, with about forty turns to repay
//! its Settler before the race ends.
//!
//! So [`CITY_WIDTH_DRAW`] puts five of its eight shares on eleven to
//! fourteen and three on fifteen, sixteen and eighteen: the wide end is the
//! operator's experiment, and no measured width yields less than ten does.
//!
//! ## What the gene does
//!
//! `assess` asks it after the shared contract, the difficulty widening and
//! `good-land-lifts-the-city-target`, so every later cut still applies:
//! `city-target-meets-the-map` (no city without a practical site) and a
//! calibrated chair's pace. The Science contract's cap rises to the drawn
//! width, or a Science seat would be cut back to its twelve.
//!
//! The target becomes the cities held plus [`CITY_WIDTH_STEP`], never above
//! the drawn width and never below what the plan already wanted. It speaks
//! only once that reaches past [`SHARED_CITY_TARGET_FLOOR`], so the opening
//! and every plan below ten are unchanged. Walking two ahead rather than
//! asking for the whole width at once keeps the Settler pipeline at the size
//! it already has: a flat target of 15 or 20 sent several Settlers out at
//! once from eight or nine cities and cost 8% of Production at turn 125 where
//! the land was not there (Deity sweep, 2026-11-09).
//!
//! The width is drawn from [`CITY_WIDTH_DRAW`] by a hash of the game's seed,
//! the seat and its civilization, so it is fixed for the game, differs
//! between the majors of one world, and does not touch the game's own random
//! stream.
//!
//! Off (the default) nothing here runs.
use super::*;

/// The widths a major may draw, one entry per equal share.
pub(crate) const CITY_WIDTH_DRAW: [usize; 8] = [11, 12, 12, 13, 14, 15, 16, 18];

/// How many cities ahead of what it holds the target may run.
pub(crate) const CITY_WIDTH_STEP: usize = 2;

impl AdvancedAi {
    /// The width `pid` builds to in this game, from [`CITY_WIDTH_DRAW`].
    pub(super) fn drawn_city_width(g: &Game, pid: usize) -> usize {
        let civ = g.players.get(pid).map_or("", |player| player.civ.as_str());
        CITY_WIDTH_DRAW[city_width_index(g.seed, pid, civ, CITY_WIDTH_DRAW.len())]
    }

    /// The plan's city target for `held` cities when it already wants
    /// `wanted`: two ahead of what it holds, to at most the drawn width, once
    /// that passes the shared ten. `wanted` unchanged with the gene off.
    pub(super) fn city_width_target(
        &self,
        g: &Game,
        pid: usize,
        wanted: usize,
        held: usize,
    ) -> usize {
        if !self.each_civ_draws_its_width {
            return wanted;
        }
        stepped_city_target(wanted, held, Self::drawn_city_width(g, pid))
    }

    /// The Science contract's `cap`, raised to the drawn width with the gene
    /// on.
    pub(super) fn city_width_science_cap(&self, g: &Game, pid: usize, cap: usize) -> usize {
        if !self.each_civ_draws_its_width {
            return cap;
        }
        cap.max(Self::drawn_city_width(g, pid))
    }
}

/// `wanted`, or `held` plus [`CITY_WIDTH_STEP`] capped at `width` when that
/// is more and passes [`SHARED_CITY_TARGET_FLOOR`].
pub(super) fn stepped_city_target(wanted: usize, held: usize, width: usize) -> usize {
    let stepped = (held + CITY_WIDTH_STEP).min(width);
    if stepped > SHARED_CITY_TARGET_FLOOR {
        wanted.max(stepped)
    } else {
        wanted
    }
}

/// A stable index below `len` for this seed, seat and civilization: FNV-1a
/// over the three, finished with the SplitMix64 mixer.
pub(super) fn city_width_index(seed: u64, pid: usize, civ: &str, len: usize) -> usize {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in seed
        .to_le_bytes()
        .into_iter()
        .chain((pid as u64).to_le_bytes())
        .chain(civ.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    hash ^= hash >> 31;
    (hash % len.max(1) as u64) as usize
}

#[cfg(test)]
mod tests;
