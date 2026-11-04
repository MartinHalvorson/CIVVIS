//! `first-campus-between-the-units`: make the first science investment
//! between fresh military builds, once a land army and other producers exist.
//!
//! In the qualified native Standard Immortal history
//! `civvis-20261104T061724Z`, a feasible first Campus waited from Writing at56
//! until placement at163. The binding hold at106 was an uninvested Heavy
//! Chariot during a major war, with three cities and six healthy land units;
//! the host offered an eleven-turn Campus. This is a development observation,
//! not a counterfactual completion, safety or stronger-play result.
//!
//! Extend the existing `campus-claims-the-free-slot` claim only when there is
//! no Campus standing or queued anywhere, at least three cities and two
//! healthy non-recon land combat units per city. A major-war military queue
//! may yield only with zero invested production and more than one turn left.
//! The caller retains its feasible-Campus picker, threatened/recently attacked
//! city guards, earlier claims, wall-breaker protection and normal production
//! legality. The first queued Campus closes this exception for other cities.
//! Requires the existing Campus claim to be enabled. Off by default.

use super::*;

const FIRST_CAMPUS_MIN_CITIES: usize = 3;
const FIRST_CAMPUS_HEALTHY_UNITS_PER_CITY: usize = 2;
const FIRST_CAMPUS_HEALTHY_HP: i32 = 50;

impl AdvancedAi {
    /// A narrowly deferrable queue for an otherwise feasible first Campus.
    /// Called after the existing earlier claims and wall-breaker guard.
    pub(super) fn first_campus_between_units_may_defer(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        at_major_war: bool,
    ) -> bool {
        if !self.first_campus_between_the_units || !at_major_war {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.owner != pid || city.queue.first() != Some(item) {
            return false;
        }
        let Item::Unit { unit } = item else {
            return false;
        };
        if self.journal().wants(crate::reasoning::Level::Detail) {
            let cities = g.player_city_ids(pid);
            let healthy: Vec<_> = g
                .player_unit_ids(pid)
                .into_iter()
                .filter(|id| Self::first_campus_healthy_land_unit(g, &g.units[id]))
                .map(|id| format!("{}:{}", g.units[&id].kind, g.units[&id].hp))
                .collect();
            let campuses: Vec<_> = cities
                .iter()
                .filter(|id| {
                    let c = &g.cities[*id];
                    g.city_has_district_family(c, crate::name!("campus"))
                        || c.queue.iter().any(|queued| {
                            matches!(queued, Item::District { district, .. }
                                if g.district_family(*district) == "campus")
                        })
                })
                .map(|id| g.cities[id].name.as_str())
                .collect();
            think!(self.journal(), Cities, Detail,
                   "{} checks a first-Campus gap", city.name;
                   "first-campus-between-the-units: {}; invested {:.1}, cost {:.1}, production {:.1}; {} cities, healthy land {:?}; Campus cities {:?}",
                   Self::plain_item(item), g.item_invested_production(cid, item),
                   g.item_cost_for_city(pid, cid, item), g.city_yields(cid).production,
                   cities.len(), healthy, campuses);
        }
        if !g
            .rules
            .units
            .get(unit)
            .is_some_and(|spec| spec.class == "military" && spec.promotion_class != "recon")
            || g.item_invested_production(cid, item) != 0.0
            || g.item_cost_for_city(pid, cid, item) <= g.city_yields(cid).production
        {
            return false;
        }
        let cities = g.player_city_ids(pid);
        if cities.len() < FIRST_CAMPUS_MIN_CITIES
            || cities.iter().any(|id| {
                let city = &g.cities[id];
                g.city_has_district_family(city, crate::name!("campus"))
                    || city.queue.iter().any(|queued| {
                        matches!(queued, Item::District { district, .. }
                            if g.district_family(*district) == "campus")
                    })
            })
        {
            return false;
        }
        let needed = FIRST_CAMPUS_HEALTHY_UNITS_PER_CITY * cities.len();
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|id| Self::first_campus_healthy_land_unit(g, &g.units[id]))
            .take(needed)
            .count()
            >= needed
    }

    fn first_campus_healthy_land_unit(g: &Game, unit: &crate::game::Unit) -> bool {
        let spec = &g.rules.units[unit.kind];
        unit.hp >= FIRST_CAMPUS_HEALTHY_HP
            && spec.class == "military"
            && spec.domain.as_deref().is_none_or(|domain| domain == "land")
            && spec.promotion_class != "recon"
            && (spec.is_melee_capable() || spec.has_ranged_attack())
    }
}

#[cfg(test)]
mod tests;
