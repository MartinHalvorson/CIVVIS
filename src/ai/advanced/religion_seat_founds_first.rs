//! `religion-seat-founds-first`: on a seat aiming at a Religious victory that
//! has not founded, the first idle, unthreatened city that can place a Holy
//! Site starts one as soon as Astrology allows it, from the first city on and
//! inside the opening book.
//!
//! **Why (live 6p Immortal, 2026-10-31).**
//! - The race is decided by t30-37. Of the last five Religion seats, the two
//!   that founded had their Holy Site at t23 (founded t38) and t34 (founded
//!   t60, a slot still open). Three lost every slot: civvis-20261031T093617Z
//!   had no Holy Site before all four Prophets were claimed at t37, 094146Z
//!   started it at t29 and 101410Z at t36.
//! - The site waits for the third city. The race package does admit a
//!   Religion seat (`prophet-race-takes-a-district-slot-2` opens "the Holy
//!   Site for the Great Prophet race"), but `expansion-before-prophet`
//!   (HostOnly) keeps the strategy at Expansion below three cities. The
//!   capital meanwhile builds Settlers at 5-9 production: 094146Z had
//!   Astrology at t9, its third city at t29 and its Holy Site at t29, and
//!   101410Z had Astrology at t10, its third city at t28 and its site at t36.
//!   `religious_production` runs only once the four-build opening book is
//!   done, and then only in an empty queue.
//! - A founding seat now lives. Under the operator's loosened retire rule
//!   (one sixth of the leader from t120), 063431Z founded at t38 and played
//!   to t179; a seat that loses the race hands its lane to Science at ~t74.
//!
//! **What the gene does.** For a Religion-target seat with no religion, no
//! Prophet in hand and a Prophet still open (`prophet_race_open_for`), when no
//! city of ours holds or queues a Holy Site, the idle city that produces most
//! among those with a Holy Site placement claims the best-Faith one. It runs
//! before the opening book's branch, after the book's held Settler has taken
//! its queue, so the district goes into the first queue that empties after
//! Astrology rather than after the third city: one Settler later, in
//! exchange for the race. It never takes a queue that holds anything, or a
//! threatened or freshly attacked city. Everything after the district (the
//! Shrine, Prayers, the Prophet) is the existing path's.
//!
//! Off, or for a seat not aiming at Religion, nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether the gene acts for this seat: on, and aiming at Religion.
    pub(super) fn religion_seat_founds_first_active(&self, g: &Game) -> bool {
        self.religion_seat_founds_first
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
    }

    fn holy_site_started(g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid).into_iter().any(|cid| {
            let city = &g.cities[&cid];
            g.city_has_district_family(city, crate::name!("holy_site"))
                || city.queue.iter().any(|item| {
                    matches!(item, Item::District { district, .. }
                        if g.district_family(*district) == "holy_site")
                })
        })
    }

    /// The idle city and Holy Site the founding wants now. `None` with the
    /// gene off, for another target, once founded or a Prophet is in hand,
    /// with the race closed, once a site stands or is queued, or when no idle,
    /// unthreatened city can place one.
    pub(super) fn religion_seat_holy_site(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        if !self.religion_seat_founds_first_active(g) {
            return None;
        }
        let player = &g.players[pid];
        if player.religion.is_some()
            || player.prophet_pending
            || !self.prophet_race_open_for(g, pid)
            || Self::holy_site_started(g, pid)
        {
            return None;
        }
        let mut best: Option<(f64, f64, u32, Item)> = None;
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            if !city.queue.is_empty()
                || plan.threatened_city == Some(cid)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            {
                continue;
            }
            let production = g.city_yields(cid).production;
            for item in g.producible_items(pid, cid) {
                let Item::District { district, pos } = &item else {
                    continue;
                };
                if g.district_family(*district) != "holy_site" {
                    continue;
                }
                let faith = g.district_yields(*district, *pos).faith;
                let better = best.as_ref().is_none_or(|(p, f, c, _)| {
                    production
                        .total_cmp(p)
                        .then(faith.total_cmp(f))
                        .then(c.cmp(&cid))
                        .is_gt()
                });
                if better {
                    best = Some((production, faith, cid, item.clone()));
                }
            }
        }
        best.map(|(_, _, cid, item)| (cid, item))
    }

    /// See the module. Exact no-op with the gene off or for another target.
    pub(super) fn claim_religion_seat_holy_site(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some((cid, item)) = self.religion_seat_holy_site(g, pid, plan) else {
            return;
        };
        let name = g.cities[&cid].name.clone();
        let cities = g.player_city_ids(pid).len();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                   "{name} starts its Holy Site for the founding";
                   "religion-seat-founds-first: {} of {} religions founded, {cities} city(ies), \
                    an idle queue",
                   g.religions_founded(), g.max_religions());
        }
    }
}

#[cfg(test)]
mod tests;
