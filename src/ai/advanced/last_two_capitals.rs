//! `the-last-two-capitals-come-first`: while Domination needs at most
//! [`LAST_CAPITALS`] more original capitals and the campaign's war target
//! holds one of them within [`super::FINISHING_CAPITAL_MARCH`] of a city of
//! ours, that capital is the campaign's first objective. The same committed
//! siege filter as the finishing capital applies, so a siege that has damaged
//! its city still finishes it.
//!
//! `domination-finish-holds-the-front` names a capital only when its capture
//! completes Domination. With two left, neither one completes it, so the
//! campaign keeps aiming at border towns while the clock runs.
//!
//! **Why (retained live games to 2026-10-26, Emperor and Immortal).**
//! - Counting only majors' original capitals, 9 games reached two or fewer
//!   capitals still to take; one won (civvis-20261025T145013Z).
//! - In the pre-arm games, 41 campaign turns from that point aimed at a
//!   remaining capital, and 139 aimed at another city of a rival that held
//!   one. That census also counted city-state capitals, so the tally is
//!   rough; the pattern is not.
//! - Immortal civvis-20261025T213230Z held Bogotá and Kyoto at turn 212 with
//!   Amsterdam and Sparta left. At war with Greece, the campaign aimed at
//!   Ephesus from 216 to 224; Sparta became the objective at 239, after
//!   Amsterdam fell. The Netherlands won on Science at 241.
//! - Armed, civvis-20261026T014409Z held two from turn 149. It aimed at the
//!   target's remaining capital on 52 of 53 such campaign turns, and took
//!   Tenochtitlan at 220.
//!
//! **Every capital must be known.** The first version dropped a living
//! major's capital that the mirror had never seen, so a four-major game
//! holding only Bogotá read as two left. Over the armed games of 2026-10-25/26,
//! 88 of 129 "Campaign holds on" turns came with three or more capitals still
//! to take, from turn 23 on. For example, 142148Z held on Tbilisi from turn 90,
//! and 132440Z on Majapahit at 36, each holding only Bogotá. Those turns aimed
//! early campaigns at walled capitals instead of frontier towns. An unseen
//! capital now makes the count unknown, and the rule stands down.
//!
//! Off, nothing changes.

use super::*;

/// Domination may still need at most this many original capitals.
pub(super) const LAST_CAPITALS: usize = 2;

impl AdvancedAi {
    /// The original capitals Domination still needs: every major's capital,
    /// ours included, that a city of ours does not hold. `None` where the
    /// count is not the plain Domination one (teams, Require-N, a finished or
    /// played-on game, Domination off).
    pub(super) fn domination_capitals_needed(g: &Game, pid: usize) -> Option<Vec<u32>> {
        if g.players
            .get(pid)
            .is_none_or(|player| !player.alive || player.is_minor || player.is_barbarian)
            || g.players[pid].team.is_some()
            || !g.effective_victory_conditions().domination
            || g.effective_required_victories() > 1
            || g.is_finished()
            || g.played_on()
        {
            return None;
        }
        let majors: Vec<usize> = g
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        if majors.len() < 2 {
            return None;
        }
        let mut needed = Vec::new();
        for major in majors {
            match g
                .cities
                .values()
                .find(|city| city.is_capital && city.original_owner == major)
            {
                Some(capital) => {
                    if capital.owner != pid {
                        needed.push(capital.id);
                    }
                }
                // A living major whose original capital we have never seen
                // still holds a capital to take, so the count is unknown.
                // `capture_completes_domination` reads it the same way.
                None if g.players[major].alive => return None,
                None => {}
            }
        }
        Some(needed)
    }

    /// The remaining capital `target` holds that the campaign aims at first:
    /// see the module. The nearest to a city of ours, then the lowest id.
    /// `None` with the gene off.
    pub(super) fn last_two_capitals_objective(
        &self,
        g: &Game,
        pid: usize,
        target: Option<usize>,
    ) -> Option<u32> {
        if !self.the_last_two_capitals_come_first
            || self.forced_target_player.is_some()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        let target = target?;
        if !g.is_at_war(pid, target) {
            return None;
        }
        let needed = Self::domination_capitals_needed(g, pid)?;
        if needed.is_empty() || needed.len() > LAST_CAPITALS {
            return None;
        }
        let ours = g.player_city_ids(pid);
        needed
            .into_iter()
            .filter(|cid| g.cities[cid].owner == target)
            .filter_map(|cid| {
                let pos = g.cities[&cid].pos;
                ours.iter()
                    .map(|city| g.wdist(g.cities[city].pos, pos))
                    .min()
                    .filter(|march| *march <= FINISHING_CAPITAL_MARCH)
                    .map(|march| (march, cid))
            })
            .min()
            .map(|(_, cid)| cid)
    }
}

#[cfg(test)]
mod tests;
