//! `industrial-zone-in-the-producers`: once Apprenticeship is in hand, the
//! empire's two or three most productive unthreatened cities with a free
//! district slot each place an Industrial Zone on their best adjacency site
//! and then build its Workshop. See `BasicAi::industrial_zone_in_the_producers`.
//!
//! Live Emperor civvis-20261006T024058Z..T114112Z (42 games, 4p Tiny
//! Pangaea, Gran Colombia): Apprenticeship came at a median t80 (t58-111),
//! the first zone a median 40 turns later (t120; never in 3 games), a second
//! in 23 games at a median t180. At t100 29 of 41 games held no zone and none
//! a Workshop; at t150 a median 1 zone, 1 Workshop and 0 Factories, with
//! Production 0.46x the best rival's. Over the 2,832 city-turns in which one
//! of the three most productive cities could have opened a zone (tech known,
//! a specialty slot free, no zone) it built soldiers 44%, ordinary buildings
//! 21%, a Campus 8%, a Settler 8%, Builders 6% and a Theater 3%. A zone city
//! with its Workshop gained a median 2.8 (mean 4.0) Production over the
//! empire's zone-less cities in the same turns (41 cities).
//!
//! The armed `industrial-hub` places one zone by construction, in the city
//! its Factory would reach most from, and only from the delegated governor:
//! it first fired at a median t121 (30 of 42 games). After Apprenticeship the
//! strategic scorer started 357 builds in those top-three cities, the
//! military reservations 133 and the delegated governor 232; the scorer has
//! no zone step of its own (`profitable_industrial_foundation` builds a
//! standing zone's buildings, never the district), and the culture-defense
//! Theater reservation claims the most productive idle Campus city first. So
//! the step is asked in all three places: the delegated governor's
//! `pick_item` (behind the industrial hub, ahead of the Commercial Hub step
//! and the military floor), the strategic governor's idle queue (beside
//! `profitable_industrial_foundation`, behind its defence, recovery, Builder,
//! Granary and Trader reservations), and the Theater reservation, which
//! leaves a producer's slot to the zone.
use super::BasicAi;
use crate::game::{Game, Item};

/// The fewest cities from which the step opens a zone.
const PRODUCER_ZONE_MIN_CITIES: usize = 3;

/// The most zones (standing, founded or queued) the step wants: two, three
/// from `PRODUCER_ZONE_THIRD_AT_CITIES` cities.
pub(crate) const PRODUCER_ZONE_MAX: usize = 3;

/// Cities from which the step wants its third zone.
const PRODUCER_ZONE_THIRD_AT_CITIES: usize = 6;

/// The slowest a city may build its zone or the zone's Workshop: a Workshop
/// is 195 production at Standard.
pub(crate) const PRODUCER_ZONE_MAX_TURNS: f64 = 20.0;

impl BasicAi {
    /// Zones the step wants standing, founded or queued.
    fn producer_zones_wanted(n_cities: usize) -> usize {
        if n_cities >= PRODUCER_ZONE_THIRD_AT_CITIES {
            PRODUCER_ZONE_MAX
        } else {
            2
        }
    }

    /// The plan's threatened city or a city attacked within four turns: the
    /// test the housing and Commercial Hub steps use.
    fn producer_city_threatened(g: &Game, cid: u32, threatened: Option<u32>) -> bool {
        let city = &g.cities[&cid];
        threatened == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
    }

    /// Whether `cid` holds an Industrial Zone, its foundation, or has one
    /// first in its queue.
    fn industrial_zone_held_or_queued(g: &Game, cid: u32) -> bool {
        let city = &g.cities[&cid];
        g.city_has_district_family(city, crate::name!("industrial_zone"))
            || city.owned_tiles.iter().any(|pos| {
                g.map
                    .get(*pos)
                    .and_then(|tile| tile.district_foundation.as_ref())
                    .is_some_and(|foundation| {
                        g.district_family(foundation.district) == "industrial_zone"
                    })
            })
            || matches!(
                city.queue.first(),
                Some(Item::District { district, .. })
                    if g.district_family(*district) == "industrial_zone"
            )
    }

    /// Turns `cid` needs for `item`: the host's figure where it gave one.
    fn producer_build_turns(g: &Game, pid: usize, cid: u32, item: &Item) -> f64 {
        g.host_production_turns(cid, item)
            .unwrap_or_else(|| g.item_cost_for(pid, item) / g.city_yields(cid).production.max(0.5))
    }

    /// `industrial-zone-in-the-producers`: this city's next production build,
    /// in order: the Workshop of the zone it holds, when it is one of the
    /// `producer_zones_wanted` most productive unthreatened zone cities; else
    /// its own zone (`producer_zone_item`). Either gives its slot to the
    /// city's standing Campus's next building -- under `campus-buildings-first`
    /// that step's Library, University or Research Lab, otherwise the Library
    /// alone. `None` while the gene is off, below three cities, in a
    /// threatened city and in a city due its Granary (population within one
    /// of its housing with a Granary buildable). The caller checks economic
    /// recovery, local defence and a due Settler.
    pub(crate) fn industrial_zone_producer_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        threatened: Option<u32>,
    ) -> Option<Item> {
        if !self.industrial_zone_in_the_producers
            || self.minor
            || self.barb
            || n_cities < PRODUCER_ZONE_MIN_CITIES
            || Self::producer_city_threatened(g, cid, threatened)
        {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if (city.pop as f64) + 1.0 >= g.city_housing(city)
            && Self::civ_building(g, pid, cid, "granary").is_some()
        {
            return None;
        }
        let item = Self::producer_workshop_item(g, pid, cid, n_cities, threatened)
            .or_else(|| Self::producer_zone_item(g, pid, cid, n_cities, threatened))?;
        let campus = if self.campus_buildings_first {
            Self::campus_building_item(g, pid, cid)
        } else {
            Self::campus_library_item(g, pid, cid)
        };
        Some(campus.unwrap_or(item))
    }

    /// See `industrial_zone_producer_item`: the Workshop (or this
    /// civilization's replacement) of the zone standing in `cid`, when `cid`
    /// is one of the `producer_zones_wanted` most productive unthreatened
    /// cities holding a zone and finishes it within `PRODUCER_ZONE_MAX_TURNS`.
    fn producer_workshop_item(
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        threatened: Option<u32>,
    ) -> Option<Item> {
        let zone = crate::name!("industrial_zone");
        if !g.city_has_district_family(g.cities.get(&cid)?, zone) {
            return None;
        }
        let workshop = Self::civ_building(g, pid, cid, "workshop")?;
        let mut ranked: Vec<(f64, u32)> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|city| {
                g.city_has_district_family(&g.cities[city], zone)
                    && !Self::producer_city_threatened(g, *city, threatened)
            })
            .map(|city| (g.city_yields(city).production, city))
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        if !ranked
            .iter()
            .take(Self::producer_zones_wanted(n_cities))
            .any(|(_, city)| *city == cid)
        {
            return None;
        }
        (Self::producer_build_turns(g, pid, cid, &workshop) <= PRODUCER_ZONE_MAX_TURNS)
            .then_some(workshop)
    }

    /// See `industrial_zone_producer_item`: this city's Industrial Zone on
    /// its site of highest Production adjacency (mines, quarries, strategic
    /// resources, an Aqueduct, Dam or Canal, other districts, as
    /// `district_yields` prices them), when the empire holds, has founded or
    /// has queued fewer zones than `producer_zones_wanted` and this city is
    /// among the most productive of the unthreatened cities that lack one,
    /// have a site (a free specialty slot) and are not due their first Campus
    /// (`first_campus_item`) -- as many as the zones still wanted -- and
    /// finishes it within `PRODUCER_ZONE_MAX_TURNS`.
    fn producer_zone_item(
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        threatened: Option<u32>,
    ) -> Option<Item> {
        let zone = Self::civ_district(g, pid, "industrial_zone");
        let spec = g.rules.districts.get(&zone)?;
        let unlocked = spec
            .tech
            .as_ref()
            .is_none_or(|tech| g.players[pid].techs.contains(tech))
            && spec
                .civic
                .as_ref()
                .is_none_or(|civic| g.players[pid].civics.contains(civic));
        if !unlocked || Self::industrial_zone_held_or_queued(g, cid) {
            return None;
        }
        let own = g.player_city_ids(pid);
        let held = own
            .iter()
            .filter(|city| Self::industrial_zone_held_or_queued(g, **city))
            .count();
        let open = Self::producer_zones_wanted(n_cities).saturating_sub(held);
        if open == 0 {
            return None;
        }
        let best_site = |city: u32| {
            g.district_sites(city, zone)
                .into_iter()
                .map(|pos| (pos, g.district_yields(zone, pos)))
                .max_by(|a, b| {
                    a.1.production
                        .total_cmp(&b.1.production)
                        .then(a.1.total().total_cmp(&b.1.total()))
                        .then(b.0.cmp(&a.0))
                })
                .map(|(pos, _)| pos)
        };
        let mut ranked: Vec<(f64, u32)> = own
            .iter()
            .copied()
            .filter(|city| {
                !Self::industrial_zone_held_or_queued(g, *city)
                    && !Self::producer_city_threatened(g, *city, threatened)
            })
            .filter(|city| best_site(*city).is_some())
            .filter(|city| Self::first_campus_item(g, pid, *city).is_none())
            .map(|city| (g.city_yields(city).production, city))
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        if !ranked.iter().take(open).any(|(_, city)| *city == cid) {
            return None;
        }
        let item = Item::District {
            district: zone,
            pos: best_site(cid)?,
        };
        (g.can_produce(pid, cid, &item)
            && Self::producer_build_turns(g, pid, cid, &item) <= PRODUCER_ZONE_MAX_TURNS)
            .then_some(item)
    }

    /// `industrial-zone-in-the-producers`: whether a queued `item` in `cid` is
    /// an Industrial Zone or a Workshop the strategic governor's review keeps
    /// against a higher bid -- the gene's build, never in a threatened city.
    /// `false` while the gene is off.
    pub(crate) fn industrial_zone_build_holds(
        &self,
        g: &Game,
        cid: u32,
        item: &Item,
        threatened: Option<u32>,
    ) -> bool {
        if !self.industrial_zone_in_the_producers
            || Self::producer_city_threatened(g, cid, threatened)
        {
            return false;
        }
        match item {
            Item::District { district, .. } => g.district_family(*district) == "industrial_zone",
            Item::Building { building } => {
                *building == "workshop"
                    || g.rules
                        .buildings
                        .get(building)
                        .is_some_and(|spec| spec.replaces == Some(crate::name!("workshop")))
            }
            _ => false,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
