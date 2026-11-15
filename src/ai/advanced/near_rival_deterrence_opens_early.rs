//! `near-rival-deterrence-opens-early`: `near-rival-deterrence` opens at
//! live turn 10 ([`EARLY_DETERRENCE_START_STANDARD`]) instead of 30, and a
//! met major at peace whose land soldier stood within [`CAPITAL_WATCH_TILES`]
//! of our original capital in the capital watch's window (ten live turns)
//! is a neighbour for the opening deterrent even when none of its cities is
//! known within [`DETERRENCE_NEAR_TILES`] of ours. Everything else is the
//! opening gene's claim unchanged: one idle or routine queue a turn trains a
//! land unit (at most two in training) while that neighbour out-guns us, and
//! the early city floor's Settler, Builders, Walls, wonders, investments and
//! every claim hold the opening gene respects keep their queues. The late
//! gene (`near-rival-deterrence-holds-late`) is not widened: past the window
//! only a known city makes a neighbour.
//!
//! **Why (live Immortal 6p Continents Online, 2026-11-15, the last four
//! original capitals lost, measured from each run's `why.log` and
//! `events.jsonl`).** Each fell to an opening war by a stronger neighbour,
//! and the opening deterrent did not train one unit against it before the
//! declaration:
//!
//! | run | war | our power / theirs | why the deterrent was silent |
//! |---|---|---|---|
//! | `215049Z` Sweden | Australia t45 | 130 / 283 | met t9, its soldiers 1-3 tiles from Stockholm from t12; no Australian city seen until t60, so no neighbour |
//! | `222126Z` Rome | Nubia t42 | 121 / 273 | met t32 with a city 10 tiles out; the walls genes held all four queues t34-41 |
//! | `222931Z` Brazil | Rome t24 | 79 / 203 | met t12 at 3.3x, its soldiers within 8 tiles; the war came before turn 30 |
//! | `223324Z` Sumer | Japan t59 | 151 / 196 | met t19, a soldier 4 tiles from Uruk at t21; the nearest Japanese city seen was 11 tiles out until t50 |
//!
//! The seat built Scouts, two to four Settlers, Monuments, Granaries,
//! Campuses and Walls in those openings, and bought Builders and Traders
//! with 90-120 Gold each; it held one Warrior and at most one Slinger or
//! Archer to turn 24 (power 34-69 against 100-200). The 10-08 deterrence
//! census in `advanced/near_rival_deterrence.rs` puts declarations at our
//! power below half of a neighbour's at 1.27 per 100 rival-turns, the
//! highest band, and three of the four games sat there.
//!
//! **Why the capital watch.** It is the memory `capital-walls-before-the-
//! surprise` already keeps (a soldier within eight tiles in the last ten
//! turns fired before 27 of 28 capital losses in its 10-13/14 census), so
//! the door needs no new observation; this gene records it too. In two of
//! the four games it was the only sign of the neighbour before the war.
//!
//! Off: nothing changes. Grep the journal for
//! `near-rival-deterrence-opens-early:`; the claims it adds read that tag
//! where the opening gene's lines read `near-rival-deterrence:`.

use super::capital_walls_before_the_surprise::{CAPITAL_WATCH_STANDARD, CAPITAL_WATCH_TILES};
use super::near_rival_deterrence::{DETERRENCE_NEAR_TILES, DETERRENCE_START_STANDARD};
use super::*;

/// The opening deterrent's first turn under this gene, in standard turns:
/// live turn 10 at Online speed (the opening gene's own is 45, turn 30).
pub(crate) const EARLY_DETERRENCE_START_STANDARD: u32 = 15;

impl AdvancedAi {
    /// The opening deterrent's first turn: [`EARLY_DETERRENCE_START_STANDARD`]
    /// with this gene on, the opening gene's [`DETERRENCE_START_STANDARD`]
    /// otherwise.
    pub(super) fn deterrence_window_start(&self, g: &Game) -> u32 {
        g.standard_duration(if self.near_rival_deterrence_opens_early {
            EARLY_DETERRENCE_START_STANDARD
        } else {
            DETERRENCE_START_STANDARD
        })
    }

    /// Whether a soldier of `rival` stood within [`CAPITAL_WATCH_TILES`] of
    /// our original capital in the capital watch's window. False with the
    /// gene off.
    pub(super) fn deterrence_door_neighbour(&self, g: &Game, rival: usize) -> bool {
        self.near_rival_deterrence_opens_early
            && self
                .capital_watch_sightings
                .get(&rival)
                .is_some_and(|seen| {
                    *seen <= g.turn && g.turn - *seen <= g.standard_duration(CAPITAL_WATCH_STANDARD)
                })
    }

    /// Whether a known city of `rival` stands within
    /// [`DETERRENCE_NEAR_TILES`] of one of ours: the opening gene's own
    /// neighbour test.
    pub(super) fn deterrence_city_near(g: &Game, ours: &[Pos], rival: usize) -> bool {
        g.cities.values().any(|city| {
            city.owner == rival
                && ours
                    .iter()
                    .any(|home| g.wdist(*home, city.pos) <= DETERRENCE_NEAR_TILES)
        })
    }

    /// The journal's reason when this gene, not the opening gene alone,
    /// armed the claim against `rival`: the turn is before the opening
    /// gene's window, or the soldier at the door is the only sign of the
    /// neighbour. `None` otherwise and with the gene off.
    pub(super) fn deterrence_opens_early_reason(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<String> {
        if !self.near_rival_deterrence_opens_early {
            return None;
        }
        let start = g.standard_duration(DETERRENCE_START_STANDARD);
        let before = g.turn < start;
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        let city_near = Self::deterrence_city_near(g, &ours, rival);
        if city_near && !before {
            return None;
        }
        let why = if city_near {
            format!("a city within {DETERRENCE_NEAR_TILES} tiles")
        } else {
            format!(
                "its soldier stood within {CAPITAL_WATCH_TILES} tiles of our capital in the \
                 last {} turns",
                g.standard_duration(CAPITAL_WATCH_STANDARD)
            )
        };
        Some(if before {
            format!(
                "{why}; turn {}, before the opening window's turn {start}",
                g.turn
            )
        } else {
            why
        })
    }
}

#[cfg(test)]
mod tests;
