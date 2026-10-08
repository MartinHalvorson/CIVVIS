//! `prophet-builds-its-site`: a Great Prophet never arrives without a Holy
//! Site to found on, and the race stops paying once the host has no Prophet
//! left to give.
//!
//! A Prophet founds only on a finished Holy Site of its owner. The race's
//! point sources did not wait for one: `race_wants_revelation` slots
//! Revelation (+2 Prophet points a turn) "Holy Site or not", while the site
//! itself opens only once `enter_prophet_race` admits the seat. Live Emperor
//! G348 (civvis-20261007T120842Z): Revelation earned the last Prophet at
//! turn 62 with no Holy Site anywhere; Maracaibo started one that turn, the
//! Prophet stood in Bogota from 62 to 68, and the Khmer won on Religion at
//! 68 holding 3 of our 4 cities (a founding converts the Holy City and leaves
//! Buddhism 2 of 4, no majority). Game 011049Z held its Prophet from turn 57
//! to 103, the site finished at 102, and France won at 127. That is 2 of the
//! 17 Religious defeats of the October 6-7 Emperor runs. The other half of
//! the waste runs the opposite way: the host reported the Prophet class
//! exhausted at a median turn 45 over 121 Emperor games, and in 032934Z and
//! 021157Z our Holy Site, Shrine and Revelation went on banking 103 and 139
//! Prophet points into a race that had closed at turn 40.
//!
//! Under the gene:
//! - a held Prophet, the race's own point income (the Revelation it wants),
//!   or points within [`PROPHET_SITE_LEAD_TURNS`] of the Prophet's cost put
//!   the Holy Site at the head of the city that builds it soonest, and the
//!   governor keeps it there (`prophet_site_committed`);
//! - Revelation is slotted only while a Holy Site is built or queued;
//! - once the host lists the Prophet class exhausted and no Prophet of ours
//!   is pending, the race is closed for this seat (`prophet_race_open_for`),
//!   which stops the race's Holy Site, its Shrine and Revelation together.
//!
//! Off: unchanged. A founder, and a seat without the host's exhaustion list
//! (headless games), keep the shipped behaviour for those parts.

use super::{AdvancedAi, StrategicPlan};
use crate::game::{Action, Game, Item};
use crate::think;

/// Turns of Prophet income at which an approaching Prophet asks for its Holy
/// Site even when no race lever is paying for it.
pub(crate) const PROPHET_SITE_LEAD_TURNS: f64 = 2.0;

impl AdvancedAi {
    /// Whether the host lists the Prophet class as exhausted for this seat:
    /// every Great Prophet the map allows is claimed. `false` without the
    /// host's list (headless), and always `false` once a Prophet of ours is
    /// pending, since that Prophet still needs its site.
    pub(super) fn prophet_class_exhausted(g: &Game, pid: usize) -> bool {
        let player = &g.players[pid];
        !player.prophet_pending
            && player
                .live_great_person_exhausted
                .as_ref()
                .is_some_and(|exhausted| exhausted.contains("prophet"))
    }

    /// `prophet-builds-its-site`: the race is closed for this seat because the
    /// host has no Prophet left for it. Read by `prophet_race_open_for`.
    pub(super) fn prophet_race_exhausted(&self, g: &Game, pid: usize) -> bool {
        self.prophet_builds_its_site
            && g.players[pid].religion.is_none()
            && Self::prophet_class_exhausted(g, pid)
    }

    fn holy_site_item(g: &Game, item: &Item) -> bool {
        matches!(item, Item::District { district, .. }
            if g.district_family(*district).as_str() == "holy_site")
    }

    /// A finished Holy Site in any city of ours.
    pub(super) fn holy_site_completed(g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid)
            .into_iter()
            .any(|cid| g.city_has_district_family(&g.cities[&cid], crate::name!("holy_site")))
    }

    /// A Holy Site built, queued anywhere, or holding paid progress in any
    /// city of ours.
    pub(super) fn holy_site_built_or_queued(g: &Game, pid: usize) -> bool {
        Self::holy_site_completed(g, pid)
            || g.player_city_ids(pid).into_iter().any(|cid| {
                let city = &g.cities[&cid];
                city.queue.iter().any(|item| Self::holy_site_item(g, item))
                    || city.production_progress.iter().any(|(key, paid)| {
                        *paid > 0.0
                            && key
                                .strip_prefix("district:")
                                .and_then(|rest| rest.split(':').next())
                                .is_some_and(|district| {
                                    g.district_family(crate::name::Name::new(district)).as_str()
                                        == "holy_site"
                                })
                    })
            })
    }

    /// Revelation's gate under the gene: a Holy Site built or queued. True
    /// with the gene off.
    pub(super) fn revelation_has_its_site(&self, g: &Game, pid: usize) -> bool {
        !self.prophet_builds_its_site || Self::holy_site_built_or_queued(g, pid)
    }

    /// Turns until our Prophet points reach the current Prophet's cost at
    /// the present income. `None` without income.
    fn prophet_eta(g: &Game, pid: usize) -> Option<f64> {
        let rate = g
            .great_person_points_per_turn(pid)
            .get("prophet")
            .copied()
            .unwrap_or(0.0);
        if rate <= 0.0 {
            return None;
        }
        let points = g.players[pid].gpp.get("prophet").copied().unwrap_or(0.0);
        Some((g.gp_cost(pid, "prophet") - points).max(0.0) / rate)
    }

    /// Why the Holy Site is due now, or `None`. A held Prophet always asks;
    /// otherwise the race must still have a Prophet for us, and either it
    /// wants Revelation's points (which wait on the site) or our points are
    /// within [`PROPHET_SITE_LEAD_TURNS`] of the cost.
    pub(super) fn prophet_site_due(&self, g: &Game, pid: usize) -> Option<String> {
        if !self.prophet_builds_its_site || self.base.minor || self.base.barb {
            return None;
        }
        let player = &g.players[pid];
        if player.religion.is_some() || Self::holy_site_completed(g, pid) {
            return None;
        }
        if player.prophet_pending {
            return Some("a Great Prophet of ours waits for a Holy Site to found on".to_string());
        }
        if Self::prophet_class_exhausted(g, pid) || !g.great_person_class_earnable(pid, "prophet") {
            return None;
        }
        if self.race_points_wanted(g, pid)
            && g.rules.policies.contains_key("revelation")
            && g.player_city_ids(pid).len() >= 2
        {
            return Some(
                "the Prophet race wants Revelation's points, and they wait on a Holy Site"
                    .to_string(),
            );
        }
        Self::prophet_eta(g, pid)
            .filter(|turns| *turns <= PROPHET_SITE_LEAD_TURNS)
            .map(|turns| format!("our Prophet points reach the cost in {turns:.1} turns"))
    }

    /// The city and Holy Site the gene reserves: one already queued (or paid
    /// into) first, else the city that builds a site soonest. A threatened
    /// or recently attacked city, a district or wonder already holding
    /// production, and an item finishing this turn keep their queue.
    pub(super) fn prophet_site_choice(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        let cities = g.player_city_ids(pid);
        for cid in &cities {
            if let Some(item) = g.cities[cid]
                .queue
                .iter()
                .find(|item| Self::holy_site_item(g, item))
            {
                return Some((*cid, item.clone()));
            }
        }
        cities
            .into_iter()
            .filter(|cid| {
                let city = &g.cities[cid];
                if plan.threatened_city == Some(*cid)
                    || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                    // `early-settler-floor`: the floor's Settler keeps its city.
                    || self.early_settler_floor_holds(g, pid, *cid, plan)
                {
                    return false;
                }
                match city.queue.first() {
                    Some(item @ Item::District { .. }) | Some(item @ Item::Wonder { .. }) => {
                        g.item_invested_production(*cid, item) <= 0.0
                    }
                    Some(Item::Building { building })
                        if g.rules
                            .buildings
                            .get(building)
                            .is_some_and(|spec| spec.wonder) =>
                    {
                        false
                    }
                    Some(item) => {
                        g.item_cost_for_city(pid, *cid, item)
                            - g.item_invested_production(*cid, item)
                            > g.city_yields(*cid).production
                    }
                    None => true,
                }
            })
            .filter_map(|cid| {
                let item = crate::ai::BasicAi::first_district_item(g, pid, cid, "holy_site")?;
                if !g.can_produce(pid, cid, &item) {
                    return None;
                }
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    g.item_cost_for_city(pid, cid, &item) / g.city_yields(cid).production.max(0.5)
                });
                Some((turns, cid, item))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, cid, item)| (cid, item))
    }

    /// Put the Holy Site at the head of its city's queue while it is due.
    /// Exact no-op with the gene off.
    pub(super) fn reserve_prophet_site(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some(why) = self.prophet_site_due(g, pid) else {
            return;
        };
        let Some((cid, item)) = self.prophet_site_choice(g, pid, plan) else {
            return;
        };
        if g.cities[&cid].queue.first() == Some(&item)
            || self.early_settler_floor_holds(g, pid, cid, plan)
        {
            return;
        }
        let displaced = g.cities[&cid].queue.first().cloned();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Faith, Decision,
                "{} puts its Holy Site first for the Great Prophet", g.cities[&cid].name;
                "prophet-builds-its-site: {why}{}",
                displaced.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))));
        }
    }

    /// The governor keeps the reserved Holy Site through rescoring while it
    /// is due. See `advanced_production`.
    pub(super) fn prophet_site_committed(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> bool {
        Self::holy_site_item(g, item)
            && g.cities[&cid].queue.first() == Some(item)
            && self.prophet_site_due(g, pid).is_some()
    }
}

#[cfg(test)]
mod tests;
