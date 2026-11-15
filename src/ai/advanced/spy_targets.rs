//! One spy per target city: the reading the spy-posting genes share.
//!
//! `science-threat-denial`, `science-denial-every-pad` and
//! `culture-denial-heist` each keep a set of target cities and want exactly
//! one spy of ours in each. They differ only in which cities, which mission
//! the host must offer and what the posting is worth; the reading itself (who
//! holds a city, whether a posting is free, where an idle spy should go) lives
//! here once.

use std::collections::BTreeSet;

use super::AdvancedAi;
use crate::game::Game;

impl AdvancedAi {
    /// Whether the host lists `mission` on `spy`'s own menu. A spy the host
    /// sent no menu for is offered nothing.
    pub(crate) fn host_lists_spy_mission(g: &Game, spy: u32, mission: &str) -> bool {
        g.host_unit_facts
            .get(&spy)
            .and_then(|facts| facts.spy_missions.as_ref())
            .is_some_and(|menu| menu.contains(mission))
    }

    /// Whether a spy of ours other than `spy` is in, or on its way to, `cid`.
    /// The engine sets `Spy::city` to the destination the turn the order is
    /// given, so one read covers the posted and the travelling agent, and the
    /// spy pass mutates the board between spies, so a second idle spy the same
    /// turn already sees the first's order.
    pub(crate) fn other_spy_of_ours_in(g: &Game, pid: usize, spy: u32, cid: u32) -> bool {
        g.spies
            .values()
            .any(|other| other.owner == pid && other.id != spy && other.city == Some(cid))
    }

    /// The spy of ours that holds `cid`: the lowest id of the uncaptured spies
    /// posted or travelling there, so of two in one city exactly one stays.
    pub(crate) fn spy_holding(g: &Game, pid: usize, cid: u32) -> Option<u32> {
        g.spies
            .values()
            .filter(|spy| spy.owner == pid && spy.captured_by.is_none() && spy.city == Some(cid))
            .map(|spy| spy.id)
            .min()
    }

    /// What posting `spy` to `cid` is worth to a gene that wants one spy in
    /// each of `targets`: `priority` for a target no other spy of ours is in or
    /// on its way to, nothing elsewhere or with the gene off (`targets` empty).
    pub(crate) fn unheld_spy_target_bonus(
        g: &Game,
        pid: usize,
        spy: u32,
        targets: &BTreeSet<u32>,
        cid: u32,
        priority: i32,
    ) -> i32 {
        if targets.contains(&cid) && !Self::other_spy_of_ours_in(g, pid, spy, cid) {
            priority
        } else {
            0
        }
    }

    /// The free target an idle `spy` standing in a foreign city should leave
    /// for: when its city is not one of `targets`, or another spy of ours holds
    /// it. The nearest target nobody holds, ties to the lower city id. A spy at
    /// home, or one the host offers `stay_for` to where it stands, stays.
    pub(crate) fn spy_repost_to_free_target(
        g: &Game,
        pid: usize,
        spy: u32,
        current_city: Option<u32>,
        targets: &BTreeSet<u32>,
        stay_for: &str,
    ) -> Option<u32> {
        let here = current_city?;
        let city = g.cities.get(&here)?;
        if city.owner == pid || Self::host_lists_spy_mission(g, spy, stay_for) {
            return None;
        }
        if targets.contains(&here) && Self::spy_holding(g, pid, here) == Some(spy) {
            return None;
        }
        targets
            .iter()
            .copied()
            .filter(|cid| *cid != here && Self::spy_holding(g, pid, *cid).is_none())
            .min_by_key(|cid| (g.wdist(city.pos, g.cities[cid].pos), *cid))
    }
}
