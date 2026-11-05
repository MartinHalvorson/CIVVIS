//! `camps-close-the-age`: a seat a camp or two short of a Normal Age clears
//! the camp before the era ends.
//!
//! Live Lv 7 Immortal, 83 runs of 2026-11-03/04: 50 seats entered the first
//! Dark Age, 30 of them one to three Era Score short of the Normal threshold.
//! In 14 of those 30 a barbarian camp stood on our own Objective Board when
//! the era ended, first seen around turns 12-24 and served by a force of one
//! to three units against a need of 38-46. A destroyed camp is a Historic
//! Moment (`MOMENT_BARBARIAN_CAMP_DESTROYED`, 2; `_NEAR_YOUR_CITY`, 3) from the
//! Ancient era through the Medieval (`Game::add_historic_moment`). The cost of
//! the miss, read at Standard across the near-threshold seats: 4.5 cities at
//! turn 200 against 7.5, and 0.79 loyalty flips a game against 0.12.
//!
//! Under the gene, while the camp's Moment still scores and the seat is one
//! to [`CAMP_AGE_SHORT`] short of its Normal threshold, every ClearCamp row:
//!
//! - gains [`CAMP_AGE_PREMIUM`] hammers of value, an Age being worth about a
//!   settled city (`SETTLER_PREMIUM`);
//! - asks at least [`CAMP_AGE_BODIES`] bodies, so a lone Warrior is no
//!   longer the force that "serves" a guarded camp;
//! - takes the era's end as its deadline once the world-era countdown runs,
//!   so the board ranks it by the turns left.
//!
//! Version two (`camps-close-the-age-2`) keeps all of that and arms the
//! party: the row asks [`CAMP_AGE_SHOOTERS`] shooter, whose blow draws no
//! return damage and which the kill plan fires before the melee finisher,
//! and [`CAMP_AGE_MARGIN`] times the strength version one asks. Live
//! 162453Z: a raised camp stood 34 turns beside one or two Warriors and a
//! Heavy Chariot, each Warrior blow on the fortified Spearman dealing ~25
//! and taking ~38, while the camp kept spawning.
//!
//! Version three (`camps-close-the-age-3`) keeps version two and adds the
//! near camp: a camp within `CAMP_NEAR_RADIUS` of our cities gets the same
//! premium and armed party in any era while it stands, whatever the Age.
//! Live Lv 8 Deity Korea Online (11-04): barbarians outclass our line from
//! turns 9-17, and the near camps stood 32-36 turns with no force on ~60%
//! of their rows while their need rose 38 -> 70 as their units upgraded.
//!
//! Off, the board is byte-identical: [`AdvancedAi::camp_age_short`] answers
//! `None` before reading anything. Tests: `objective_board/camp_age_tests.rs`.

use super::{AdvancedAi, Game};

/// The most Era Score a seat may be short and still read camps as the way
/// to close it: two camps' worth.
pub(super) const CAMP_AGE_SHORT: i64 = 6;
/// Hammers an Age is worth on a ClearCamp row.
pub(super) const CAMP_AGE_PREMIUM: f64 = 200.0;
/// Bodies a ClearCamp row asks for while it closes an Age.
pub(super) const CAMP_AGE_BODIES: usize = 2;
/// `camps-close-the-age-2`: the age-closing party's strength over the
/// guard's, on top of `CAMP_MARGIN`.
pub(super) const CAMP_AGE_MARGIN: f64 = 1.5;
/// `camps-close-the-age-2`: shooters the age-closing party brings.
pub(super) const CAMP_AGE_SHOOTERS: usize = 1;
/// The Moment a destroyed camp scores.
const CAMP_MOMENT: &str = "MOMENT_BARBARIAN_CAMP_DESTROYED";

impl AdvancedAi {
    /// `camps-close-the-age`: Era Score the seat is short of a Normal Age,
    /// when that is one to [`CAMP_AGE_SHORT`] and a destroyed camp still
    /// scores in the world's era. `None` with the gene off.
    pub(super) fn camp_age_short(&self, g: &Game, pid: usize) -> Option<i64> {
        if !(self.camps_close_the_age || self.camps_close_the_age_2 || self.camps_close_the_age_3) {
            return None;
        }
        let spec = g.rules.historic_moments.get(CAMP_MOMENT)?;
        let scores = spec.era_score > 0
            && spec.minimum_game_era.is_none_or(|era| g.world_era >= era)
            && spec.maximum_game_era.is_none_or(|era| g.world_era <= era)
            && spec.obsolete_era.is_none_or(|era| g.world_era < era);
        if !scores {
            return None;
        }
        let player = &g.players[pid];
        let short = player.normal_age_threshold - player.era_score;
        (1..=CAMP_AGE_SHORT).contains(&short).then_some(short)
    }

    /// `camps-close-the-age`: turns until the era ends, once the world-era
    /// countdown runs. Never under one.
    pub(super) fn camp_age_deadline(&self, g: &Game) -> Option<u32> {
        let end = g.world_era_countdown_end?;
        Some(end.saturating_sub(g.turn).max(1))
    }
}
