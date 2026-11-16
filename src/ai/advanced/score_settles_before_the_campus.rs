//! `score-seat-settles-before-the-campus`: under twelve cities, a Score
//! seat's holdable-land Settler goes ahead of a Campus, Library or University
//! at the head of a city's queue.
//!
//! ## Why (21 live Immortal Rome Score games, 2026-11-15/16, 6p Continents Online, cap 180)
//!
//! - **The score follows the city count.** Final score over the leader's read
//!   0.71-1.04 at 12-13 cities (the one win held 12) and 0.40-0.63 at 6-8.
//! - **The Settler claim passes the cities over for their queue.** From turn
//!   40 to 120 the holdable-land claim's decline line ("No Settler for
//!   holdable land ... cities passed over") read "its queue keeps its item"
//!   734 times, ahead of "already training a Settler" 495, "too small" 355
//!   and "a raider in its ring" 196.
//! - **The heads it keeps are science.** On those turns the cities of
//!   population three or more not on a Settler were building: idle 21% (a
//!   snapshot between frames), the Campus district 21%, the Library 11%, a
//!   Builder 7%, Walls 5%, the Bath 4%, a Granary 4%, the University 3%. The
//!   Campus-first claims (`campus-and-library-before-the-claims`,
//!   `campus-claims-the-free-slot`) hold those heads against both Settler
//!   claims from four or six cities.
//! - **The land is there.** At turn 100 there were 35-127 unowned tiles
//!   within eight tiles of our cities in most games.
//!
//! ## What the gene does
//!
//! Only on a seat whose victory target is Score, only before turn
//! [`SCORE_SETTLES_FIRST_ONLINE_TURN`] Online (scaled to the speed), and only
//! while the empire's founded cities plus walking Settlers number fewer than
//! [`SCORE_SETTLES_FIRST_CITY_TARGET`]:
//!
//! 1. **The Settler passes the science head.** In a city the holdable-land /
//!    `no-city-limit` claim otherwise accepts (population, raider, threat,
//!    war, seat and walker tests unchanged), a Campus-family district, a
//!    Library or a University at the head of the queue no longer keeps it,
//!    unless it finishes within [`SCORE_SETTLES_FIRST_KEEP_TURNS`] turns. The
//!    displaced item keeps its progress and waits behind the Settler. Every
//!    other head keeps its queue as before.
//! 2. **The Settler stays first.** `campus-and-library-before-the-claims`
//!    would put a first Campus or a Library back ahead of a Settler with at
//!    most half its cost paid (from six and four cities), which runs after
//!    the Settler claim in the same turn and would take the queue straight
//!    back. Inside the window it leaves the Settler at the head.
//!
//! Journal reasons start `score-seat-settles-before-the-campus:` and name
//! the displaced item. Off, an exact no-op.
use super::*;

/// The empire's founded cities plus walking Settlers under which the gene
/// acts. Live, the final score over the leader's was 0.71-1.04 at 12-13
/// cities.
pub(crate) const SCORE_SETTLES_FIRST_CITY_TARGET: usize = 12;

/// The Online turn the gene closes at; scaled to the game speed by
/// [`AdvancedAi::score_settles_first_close_turn`].
pub(crate) const SCORE_SETTLES_FIRST_ONLINE_TURN: u32 = 120;

/// A science head this many turns or fewer from done keeps its queue: the
/// two turns `paused-district-finishes` and `near-done-work-finishes` use to
/// put saved work straight back at the head.
pub(crate) const SCORE_SETTLES_FIRST_KEEP_TURNS: f64 = 2.0;

impl AdvancedAi {
    /// The game turn the gene closes at: [`SCORE_SETTLES_FIRST_ONLINE_TURN`]
    /// scaled from Online to this game's speed.
    pub(super) fn score_settles_first_close_turn(g: &Game) -> u32 {
        (SCORE_SETTLES_FIRST_ONLINE_TURN as u64 * g.game_speed.duration_percent() as u64
            / crate::setup::GameSpeed::Online.duration_percent() as u64) as u32
    }

    /// Whether the gene is on and its window is open for `pid`: a Score
    /// target, before the close turn, and fewer than
    /// [`SCORE_SETTLES_FIRST_CITY_TARGET`] founded cities plus walking
    /// Settlers. Always false with the gene off.
    pub(super) fn score_settles_first_open(&self, g: &Game, pid: usize) -> bool {
        self.score_seat_settles_before_the_campus
            && self.victory_target == Some(VictoryTarget::Score)
            && g.turn < Self::score_settles_first_close_turn(g)
            && g.player_city_ids(pid).len() + Self::early_settler_census(g, pid).0
                < SCORE_SETTLES_FIRST_CITY_TARGET
    }

    /// Whether `item` is one the gene's Settler passes: a Campus-family
    /// district, a Library or a University (or a unique replacement).
    pub(super) fn score_settles_first_science_head(g: &Game, item: &Item) -> bool {
        match item {
            Item::District { district, .. } => g.district_family(*district) == "campus",
            Item::Building { .. } => matches!(
                super::campus_building_claims_the_queue::campus_building_rank(g, item),
                Some(0 | 1)
            ),
            _ => false,
        }
    }

    /// The head of `cid`'s queue the Settler claim may pass under the gene:
    /// a science head (see [`Self::score_settles_first_science_head`]) more
    /// than [`SCORE_SETTLES_FIRST_KEEP_TURNS`] turns from done, while the
    /// window is open. `None` with the gene off.
    pub(super) fn score_settles_first_passes(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<Item> {
        if !self.score_settles_first_open(g, pid) {
            return None;
        }
        let item = g.cities.get(&cid)?.queue.first()?;
        (Self::score_settles_first_science_head(g, item)
            && self.production_build_turns(g, pid, cid, item) > SCORE_SETTLES_FIRST_KEEP_TURNS)
            .then(|| item.clone())
    }
}

#[cfg(test)]
mod tests;
