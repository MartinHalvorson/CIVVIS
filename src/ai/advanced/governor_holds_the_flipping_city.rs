//! `governor-holds-the-flipping-city`: a Governor moves to a city of ours that
//! is losing Loyalty fast enough to flip, when its +8 a turn would stop the
//! slide and its current post stays loyal without it.
//!
//! ## Why
//!
//! Live Lv 7 Immortal Standard (Gran Colombia domination, 2026-11-04): captured
//! cities were lost to Loyalty, not to recapture. In `civvis-20261104T121104Z`
//! the kept captures Thebes, Nekhen, Apu (pop 10) and Tushpa (pop 14) flipped
//! at Loyalty 3-7 with no damage, at -3 to -10.5 a turn; that game captured 5
//! cities and lost 7. Across the day's games 10 captured cities and 10 of our
//! own flipped this way (session -b6's census of `city_lost` events, checked
//! by -af). The keep-or-raze decision reads Loyalty only at the capture
//! (Tushpa: "Loyalty 50 at +20.0 a turn"), and Governors are placed only at
//! appointment: Victor was re-posted to Caracas and Cumaná while Tushpa bled.
//! `Game::loyalty_change_for_city` gives a city with an active Governor +8
//! Loyalty a turn (plus policies), which would have held every one of those
//! cities losing 10.5 or less a turn.
//!
//! ## What the gene does
//!
//! Each turn, the city of ours with the fewest turns left before it reaches
//! zero Loyalty, within [`FLIP_HORIZON_STANDARD`] standard turns, that has no
//! Governor and whose rate plus [`GOVERNOR_LOYALTY`] is at least
//! [`SAVABLE_RATE`], takes a Governor: an unassigned one first, else the
//! Governor whose post keeps a non-negative rate and at least
//! [`DONOR_MIN_LOYALTY`] Loyalty without it, the post with the most slack
//! first, and never one that has not yet established at its current post (the
//! live mirror dates an established Governor exactly `establish_turns` back).
//! One move a turn. `Kind::OptIn`; off, nothing runs.

use super::*;

/// A city flipping within this many standard turns is a candidate.
pub(crate) const FLIP_HORIZON_STANDARD: u32 = 30;
/// The Loyalty an active Governor adds a turn (`Game::loyalty_change_for_city`,
/// before policies).
pub(crate) const GOVERNOR_LOYALTY: f64 = 8.0;
/// A city whose rate with a Governor stays below this is beyond saving by one.
pub(crate) const SAVABLE_RATE: f64 = -0.5;
/// A donor post must keep at least this much Loyalty without its Governor.
pub(crate) const DONOR_MIN_LOYALTY: f64 = 75.0;

impl AdvancedAi {
    /// The city to hold and the Governor to send there, or `None`. `None` with
    /// the gene off.
    pub(in crate::ai::advanced) fn flipping_city_governor_move(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<(Name, u32)> {
        if !self.governor_holds_the_flipping_city || self.base.minor || self.base.barb {
            return None;
        }
        let roster = &g.players[pid].governor_roster;
        if roster.is_empty() {
            return None;
        }
        let governed: BTreeSet<u32> = roster.values().filter_map(|state| state.city).collect();
        let horizon = f64::from(g.standard_duration(FLIP_HORIZON_STANDARD));
        // (turns to zero, city) for the most urgent savable city.
        let mut target: Option<(f64, u32)> = None;
        for cid in g.player_city_ids(pid) {
            if governed.contains(&cid) {
                continue;
            }
            let city = &g.cities[&cid];
            let rate = g.city_loyalty_per_turn(city);
            if rate >= 0.0 || rate + GOVERNOR_LOYALTY < SAVABLE_RATE {
                continue;
            }
            let turns = city.loyalty.max(0.0) / -rate;
            if turns > horizon {
                continue;
            }
            if target
                .as_ref()
                .is_none_or(|(old, old_city)| turns < *old || (turns == *old && cid < *old_city))
            {
                target = Some((turns, cid));
            }
        }
        let (_, city) = target?;
        // An unassigned, recovered Governor goes first.
        if let Some(name) = roster
            .iter()
            .filter(|(_, state)| state.city.is_none() && state.disabled_until <= g.turn)
            .map(|(name, _)| *name)
            .min()
        {
            return Some((name, city));
        }
        // Else the posted Governor whose city holds best without it.
        roster
            .iter()
            .filter(|(name, state)| {
                let establish = g
                    .rules
                    .governors
                    .get(name.as_str())
                    .map_or(0, |spec| g.standard_duration(spec.establish_turns));
                state.disabled_until <= g.turn && state.assigned_turn + establish <= g.turn
            })
            .filter_map(|(name, state)| {
                let post = g.cities.get(&state.city?)?;
                if post.owner != pid {
                    return None;
                }
                let without = g.city_loyalty_per_turn(post) - GOVERNOR_LOYALTY;
                (without >= 0.0 && post.loyalty >= DONOR_MIN_LOYALTY)
                    .then_some(((without * 100.0).round() as i64, *name))
            })
            .max_by(|left, right| left.0.cmp(&right.0).then(right.1.cmp(&left.1)))
            .map(|(_, name)| (name, city))
    }

    /// See the module. True when a Governor was moved. Exact no-op with the
    /// gene off.
    pub(in crate::ai::advanced) fn governor_holds_the_flipping_city(
        &self,
        g: &mut Game,
        pid: usize,
    ) -> bool {
        let Some((governor, city)) = self.flipping_city_governor_move(g, pid) else {
            return false;
        };
        let (name, loyalty, rate) = {
            let target = &g.cities[&city];
            (
                target.name.clone(),
                target.loyalty,
                g.city_loyalty_per_turn(target),
            )
        };
        if g.apply(pid, &Action::ReassignGovernor { governor, city })
            .is_err()
        {
            return false;
        }
        think!(self.journal(), Government, Decision,
            "Moving {} to hold {}", plain(governor.as_str()), name;
            "governor-holds-the-flipping-city: Loyalty {:.0} at {:+.1} a turn would reach zero in {:.0} turns; a Governor adds {:.0}",
            loyalty, rate, loyalty.max(0.0) / -rate, GOVERNOR_LOYALTY);
        true
    }
}

#[cfg(test)]
mod tests;
