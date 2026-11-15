//! `score-seat-founds-early`: on every seat but a Domination one, an early
//! Settler that has walked past its patience, or that the live home corridor
//! would send back to the capital, takes the best safe, holdable site within
//! reach and founds there instead of chasing its ranked site.
//!
//! **Every target but Domination** (2026-11-13: the Rome lane left the Score
//! target for Science and Diplomatic games of 250 turns, and this gene, armed
//! for Score, went inert with the target). The first six Science games
//! (`civvis-20261113T160115Z` to `T171033Z`) held 10, 3, 5, 8, 5 and 10
//! cities at turn 100. `T161411Z` sat at three cities from turn 30 to turn
//! 135 without a war while its Settlers stood unfounded: two at turn 45, four
//! at turn 75, seven from turn 105 on, under "HELD short of", "keeps (0, 37)
//! through a blocked approach" and "holds for a naval escort" for 85 turns.
//! `T165456Z` held five cities from turn 60 to turn 180 with two or three
//! Settlers on the board throughout. Over 88 Rome games a Settler first seen
//! by turn 30 outlived twenty turns 4% of the time. A Settler that walks is
//! the same loss on any peaceful lane; a Domination seat keeps its own
//! settling (its Settlers follow the conquest opening).
//!
//! ## What the live games say
//!
//! Deity Rome, 6-player Continents, Online, turn limit 170, Score target:
//! the 11 games `civvis-20261112T190022Z` to `T212311Z` (reloads folded in).
//! The final score follows how early the cities stood, and the gap is in the
//! walk, not in the production:
//!
//! - Settlers were born at the same pace in the three fastest games (8 cities
//!   by t64-t69) and in the seven slow ones: Settler #2 at a median t13, #3
//!   at t20 and #5 at t44 in both. By t80 the fast games had trained 8.0 a
//!   game and the slow ones 6.1, because they had fewer cities to train in.
//! - **From birth to founding**: a mean of 8.7 turns in the fast games (3 of
//!   24 Settlers over 15) against 23.8 in the slow ones (14 of 39 over 15).
//!   Per Settler the slow games spent 11.2 turns marching against 5.6, 3.5
//!   turns held by the safe-step guard against 1.1, and switched targets 3.9
//!   times against 1.2. Over all 11 games, 727 Settler-turns were spent past
//!   a Settler's eighth turn out (66 a game). No Settler was bought.
//! - `civvis-20261112T210532Z`: Settlers from t14; the first founding at
//!   t92. One stood "HELD short of (41, 31) | 2 tiles away and it did not
//!   move" from t17, the stall counter set its sites aside after three
//!   stalled turns (each retarget 7-8 tiles out), and from t43 to t66 it
//!   walked back toward the capital 80 times ("the live safety corridor is
//!   6 tiles"). The armed stall genes never reached it: `settler-walk-
//!   deadline` and `stalled-settler-takes-a-safe-site` count turns *out of a
//!   city* (a return home stops the count; the takeover waits 20 Online
//!   turns out), and `settler-founds-under-raiders` acts only against a
//!   barbarian.
//!
//! ## What the gene does
//!
//! On every seat but a Domination one
//! ([`AdvancedAi::score_seat_founds_early_on`]), in the first
//! [`SCORE_FOUNDS_EARLY_UNTIL_STANDARD`] standard turns (80 at Online),
//! before the ordinary Settler step:
//!
//! - The Settler's clock is the turns since it was first stepped
//!   (`settler_walk_started`), which a return home does not reset.
//! - Once it reaches [`SCORE_FOUNDS_EARLY_PATIENCE_STANDARD`] standard turns
//!   (8 at Online, the fast games' median walk), or at once when the early
//!   home corridor would turn it back toward the capital, it asks
//!   `stalled_settler_site`: the best legal site within four tiles whose
//!   ground and first step are out of every visible hostile's reach, that
//!   clears half the worth of the site it was walking to, and that passes
//!   the founding guards (`settler_legal_sites_within`: the Loyalty verdict
//!   and the revolt forecast, a rival's reserved site, capture scars).
//! - It founds at once when that site is underfoot, otherwise it retargets
//!   to it and takes a safe step. When nothing qualifies the ordinary step
//!   runs untouched: a Settler captured is worse than a Settler late.
//!
//! Off, on a Domination target, after the early window or embarked, it
//! returns before reading the board. Grep the journal for
//! `score-seat-founds-early:`.

use super::*;

/// Standard turns of a Settler's walk before a Score seat stops chasing its
/// ranked site: 8 at Online, the median walk of the three fastest games.
pub(super) const SCORE_FOUNDS_EARLY_PATIENCE_STANDARD: u32 = 10;
/// The early window, in standard turns: 80 at Online.
pub(super) const SCORE_FOUNDS_EARLY_UNTIL_STANDARD: u32 = 120;

impl AdvancedAi {
    /// The gene is on and the seat's victory target is not Domination. See
    /// the module.
    pub(super) fn score_seat_founds_early_on(&self, g: &Game) -> bool {
        self.score_seat_founds_early
            && self.active_victory_target(g) != Some(VictoryTarget::Domination)
    }

    /// `score-seat-founds-early`: the Settler's turn when its patience has
    /// run out (or the corridor would send it home) and a safe site is in
    /// reach: `Some(acted)`. `None` leaves the ordinary step untouched.
    pub(super) fn score_seat_founds_early_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.score_seat_founds_early_on(g)
            || g.turn > g.standard_duration(SCORE_FOUNDS_EARLY_UNTIL_STANDARD)
        {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.owner != pid
            || unit.kind != "settler"
            || unit.moves_left <= 0.0
            || g.is_embarked(unit)
        {
            return None;
        }
        let started = *self.settler_walk_started.entry(uid).or_insert(g.turn);
        let walked = g.turn.saturating_sub(started);
        let patience = g
            .standard_duration(SCORE_FOUNDS_EARLY_PATIENCE_STANDARD)
            .max(1);
        let homeward = self.early_settler_homeward_target(g, uid).is_some();
        if walked < patience && !homeward {
            return None;
        }
        // The takeover: freeze the plan's value floor before our pick
        // replaces the plan, as `stalled-settler-takes-a-safe-site` does.
        if !self.stalled_settler_floor.contains_key(&uid) {
            let floor = self.stalled_settler_plan_floor(g, pid, uid);
            self.stalled_settler_floor.insert(uid, floor);
        }
        let (site, worth) = self.stalled_settler_site(g, pid, uid)?;
        let here = g.units[&uid].pos;
        let why = if homeward {
            "the home corridor would turn it back toward the capital".to_string()
        } else {
            format!("{walked} turns since it set out, against a patience of {patience}")
        };
        if site == here {
            if !g.can_found_city(uid) {
                return None;
            }
            self.settler_targets.remove(&uid);
            self.settler_relaxed_targets.remove(&uid);
            self.settler_stalls.remove(&uid);
            self.settler_blocked_turns.remove(&uid);
            self.settler_closest.remove(&uid);
            self.stalled_settler_floor.remove(&uid);
            let founded = g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
            if founded {
                think!(self.journal(), Expansion, Decision,
                       "Founding a city at {here:?} for the score";
                       "score-seat-founds-early: {why}; the site underfoot is worth {worth:.1}, \
                        out of every visible hostile's reach, and a city now scores where the \
                        ranked site only might"; here);
            }
            return Some(founded);
        }
        if self.settler_targets.get(&uid) != Some(&site) {
            let distance = g.wdist(here, site);
            think!(self.journal(), Expansion, Detail,
                   "Settler takes the near site at {site:?} for the score";
                   "score-seat-founds-early: {why}; {distance} tiles away, worth {worth:.1} net \
                    of the walk, its ground and first step out of every visible hostile's reach";
                   site);
        }
        self.settler_targets.insert(uid, site);
        self.settler_relaxed_targets.insert(uid, site);
        self.settler_stalls.remove(&uid);
        self.settler_closest.remove(&uid);
        if self.settler_step_toward_safe(g, pid, uid, site) {
            return Some(true);
        }
        None
    }
}

#[cfg(test)]
mod tests;
