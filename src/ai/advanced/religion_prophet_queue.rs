//! `religion-seat-earns-its-prophet`: keep the completed Holy Site in the race.
//!
//! Native Deity Khmer `civvis-20261109T031713Z` completed its first Holy Site
//! at t17. The next idle queue trained a Settler, another Settler at t22,
//! Walls at t23, then a Builder at t27. All four religions were founded by
//! t28; our seat never founded. Starting the district early does not suffice
//! when expansion owns every subsequent queue.
//!
//! For an unfounded Religion-target seat with a Prophet still available,
//! reserve one idle, unthreatened completed Holy Site before the opening book
//! and ordinary expansion. Use the existing religion lane's order: the first
//! Shrine while no religion is founded, otherwise Holy Site Prayers. The
//! fastest eligible city gets the claim. Never replace a queue or borrow a
//! threatened or recently attacked city's production. The held opening-book
//! Settler has already taken its queue before this claim runs.
//!
//! This is independent of starting the site and exact no-op off, for another
//! target, once founded, with a Prophet in hand, or when the host closes the
//! race. It is proposal policy, not evidence of a won Prophet or game.

use super::*;

impl AdvancedAi {
    pub(super) fn religion_prophet_queue(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        if !self.religion_seat_earns_its_prophet
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
            || g.players[pid].religion.is_some()
            || g.players[pid].prophet_pending
            || Self::prophet_class_exhausted(g, pid)
            || !self.prophet_race_open_for(g, pid)
        {
            return None;
        }
        let cities = g.player_city_ids(pid);
        let has_shrine = cities.iter().any(|cid| {
            g.cities[cid]
                .buildings
                .iter()
                .any(|building| g.building_is_family(*building, crate::name!("shrine")))
        });
        let wants_shrine = !has_shrine && g.religions_founded() == 0;
        if wants_shrine
            && cities.iter().any(|cid| {
                g.cities[cid].queue.iter().any(|item| {
                    matches!(item, Item::Building { building }
                        if g.building_is_family(*building, crate::name!("shrine")))
                })
            })
        {
            return None;
        }
        let item = if wants_shrine {
            Item::Building {
                building: crate::name!("shrine"),
            }
        } else {
            Item::Project {
                project: crate::name!("holy_site_prayers"),
            }
        };
        cities
            .into_iter()
            .filter(|cid| {
                let city = &g.cities[cid];
                city.queue.is_empty()
                    && plan.threatened_city != Some(*cid)
                    && !(city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                    && g.city_has_district_family(city, crate::name!("holy_site"))
                    && g.can_produce(pid, *cid, &item)
            })
            .max_by(|left, right| {
                g.city_yields(*left)
                    .production
                    .total_cmp(&g.city_yields(*right).production)
                    .then_with(|| right.cmp(left))
            })
            .map(|cid| (cid, item))
    }

    pub(super) fn claim_religion_prophet_queue(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some((city, item)) = self.religion_prophet_queue(g, pid, plan) else {
            return;
        };
        let city_name = g.cities[&city].name.clone();
        if g.apply(pid, &Action::Produce { city, item }).is_ok() {
            think!(self.journal(), Faith, Decision,
                "{city_name} keeps earning its Prophet";
                "religion-seat-earns-its-prophet: the race is still open; reserve an idle Holy Site");
        }
    }
}

#[cfg(test)]
mod tests;
