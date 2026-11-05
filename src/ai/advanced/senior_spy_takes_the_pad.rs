//! `senior-spy-takes-the-pad`: our highest-level free Spy takes the post in
//! the leading space racer's Spaceport city from a lower-level Spy, which
//! moves on to another of the leader's cities.
//!
//! Each Spy picks its own post (`racer_spy_post_for`): a Spy standing in a
//! racer's city stays, and every other Spy skips a city one of ours already
//! holds. So whichever Spy reached the pad city first is the one that runs
//! the Disrupt, however green. The host's own odds (live 114422Z against
//! India, 2026-11-05) are 0.371 success and 0.363 failure for a level-1 Spy
//! and 0.625 and 0.277 for a level-4 one. Spies level since #4536 lets them
//! take the host's promotions: every Deity game before it peaked at level 1;
//! the five since reached levels 2 to 4, and a senior and a level-1 Spy stood
//! side by side on most turns from 150 (39 of 50 in 122513Z, 32 of 32 in
//! 125414Z).
//!
//! Under the gene, once a turn: the leader's seen-pad city that a Spy of
//! ours holds, the holder free (no order of ours still running: a Disrupt in
//! progress is never cut short) and a free Spy of a higher level outside
//! every seen pad city, not captured. The holder goes to a free leader city
//! (a Campus we have seen first, then the most populous), and the senior goes
//! to the pad city. At any stage: the one Deity win (122513Z) ran 14 of its
//! 17 Disrupts against rivals with one or two projects landed and froze Mali
//! at two, so the senior is wanted on the pad from the first launch on.
//! Off: unchanged.

use super::*;
use crate::game::Spy;

impl AdvancedAi {
    /// Whether `senior-spy-takes-the-pad` is on.
    pub fn senior_spy_takes_the_pad_enabled(&self) -> bool {
        self.senior_spy_takes_the_pad
    }

    /// The swap, once a turn, before the per-Spy pass. Returns whether one
    /// was made. Never with the gene off.
    pub(super) fn senior_spy_swap_pass(
        &mut self,
        g: &mut Game,
        pid: usize,
        leader: Option<usize>,
        ids: &[u32],
    ) -> bool {
        if !self.senior_spy_takes_the_pad {
            return false;
        }
        // The racer `racer-keeps-the-spies` names, else every major rival,
        // the most space projects landed first: the live seat runs its
        // denial through `science-denial-every-pad`, with no named leader.
        let owners: Vec<usize> = match leader {
            Some(leader) => vec![leader],
            None => {
                let mut rivals: Vec<usize> = (0..g.players.len())
                    .filter(|seat| {
                        *seat != pid && !g.players[*seat].is_minor && !g.players[*seat].is_barbarian
                    })
                    .collect();
                rivals.sort_by_key(|seat| {
                    (
                        std::cmp::Reverse(Self::science_denial_stages(g, *seat)),
                        *seat,
                    )
                });
                rivals
            }
        };
        let Some((holder, senior, pad, post)) = self.senior_spy_swap(g, pid, &owners, ids) else {
            return false;
        };
        let owner = g.cities[&pad].owner;
        let landed = Self::science_denial_stages(g, owner);
        let assign = |g: &Game, spy: u32, city: u32| {
            g.legal_spy_actions(pid, spy)
                .into_iter()
                .find(|action| matches!(action, Action::AssignSpy { city: to, .. } if *to == city))
        };
        let Some(leave) = assign(g, holder, post) else {
            return false;
        };
        if g.apply(pid, &leave).is_err() {
            return false;
        }
        let patience = g.turn + g.standard_duration(SPY_TRAVEL_ORDER_PATIENCE);
        self.spy_orders_until.insert(holder, patience);
        let Some(take) = assign(g, senior, pad) else {
            return true;
        };
        if g.apply(pid, &take).is_err() {
            return true;
        }
        self.spy_orders_until.insert(senior, patience);
        *g.players[pid]
            .counters
            .entry("senior_spy_takes_the_pad".to_string())
            .or_insert(0) += 1;
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let city = g.cities[&pad].name.clone();
            let civ = g.players[owner].civ.clone();
            let (senior_level, holder_level) = (g.spies[&senior].level, g.spies[&holder].level);
            think!(self.journal(), Military, Decision,
                   "A level {} Spy takes the post at {city} from a level {} Spy",
                   senior_level + 1, holder_level + 1;
                   "senior-spy-takes-the-pad: {civ} has landed {landed} space project(s); the \
                    senior's Disrupt odds are the better ones");
        }
        true
    }

    /// The swap to make, if any: `(holder, senior, pad city, holder's new
    /// post)`, over `owners`' seen-pad cities in order.
    pub(super) fn senior_spy_swap(
        &self,
        g: &Game,
        pid: usize,
        owners: &[usize],
        ids: &[u32],
    ) -> Option<(u32, u32, u32, u32)> {
        let ours: Vec<&Spy> = ids
            .iter()
            .filter_map(|id| g.spies.get(id))
            .filter(|spy| spy.owner == pid && spy.captured_by.is_none())
            .collect();
        let free = |spy: &Spy| !self.spy_orders_until.contains_key(&spy.id);
        let held = |city: u32| ours.iter().any(|spy| spy.city == Some(city));
        let on_a_pad = |spy: &Spy| {
            spy.city
                .is_some_and(|city| Self::racer_spy_seen_pad(g, pid, city))
        };
        for (leader, pad) in owners.iter().flat_map(|leader| {
            g.player_city_ids(*leader)
                .into_iter()
                .map(move |pad| (*leader, pad))
        }) {
            if !Self::racer_spy_seen_pad(g, pid, pad) {
                continue;
            }
            let Some(holder) = ours.iter().copied().find(|spy| spy.city == Some(pad)) else {
                continue;
            };
            if !free(holder) {
                continue;
            }
            let Some(senior) = ours
                .iter()
                .copied()
                .filter(|spy| {
                    spy.id != holder.id && free(spy) && spy.level > holder.level && !on_a_pad(spy)
                })
                .max_by_key(|spy| (spy.level, std::cmp::Reverse(spy.id)))
            else {
                continue;
            };
            let legal = g.legal_spy_actions(pid, holder.id);
            let Some(post) = g
                .player_city_ids(leader)
                .into_iter()
                .filter(|city| *city != pad && !held(*city))
                .filter(|city| {
                    legal.iter().any(
                        |action| matches!(action, Action::AssignSpy { city: to, .. } if to == city),
                    )
                })
                .max_by_key(|city| {
                    (
                        Self::racer_spy_seen_campus(g, pid, *city),
                        g.cities[city].pop,
                        std::cmp::Reverse(*city),
                    )
                })
            else {
                continue;
            };
            return Some((holder.id, senior.id, pad, post));
        }
        None
    }
}

#[cfg(test)]
mod tests;
