//! `plant-reads-the-grid`: a fuel-burning power plant is worth the powered
//! yields of every city it can fuel in reach.
//!
//! **The finding (live Deity, 2026-11-04/05).** From t150 the median science
//! seat met about half its Power demand. At t170, 24 of the 32 games that
//! needed Power were short; 9 held no plant at all, and in 7 of those a plant
//! stood in the host's buildable list. Five of the seven were banking Oil with
//! nothing to burn it: 034948Z and 105753Z sat at the 25 Oil cap and 165539Z
//! held 15, each with six Research Labs dark. 034948Z's first plant started at
//! t171.
//!
//! **The cause.** `power_switched_on` reads a plant as 4 Power for its own city
//! and pays nothing if that leaves the city short. A plant requires a Factory
//! (2 Power) and a Research Lab draws 3, so the city most worth powering has a
//! demand of 5 and the credit is exactly zero. The host has no such cap
//! (`Game::process_power`): every city within six tiles of a plant burns as
//! many units of its fuel as it needs, at 4 Power a unit. Live 173830Z matches
//! to the point: 3 Coal a turn gave 9 Power, a city of 5 and a city of 4.
//!
//! **The gene.** A plant is credited the `powered_*` yields of each city of
//! ours in its reach that no burning plant already serves, for as many of them
//! as its fuel can feed: income plus [`PLANT_FUEL_HORIZON_TURNS`]'s share of the
//! stockpile, after the cities that fuel already feeds. A city it could only
//! part power earns nothing, because `city_is_powered` needs the whole demand.
//! With no fuel the credit is zero, so the gene never buys a plant it cannot
//! run, and a Diplomacy seat's refusal of fossil power stands.
//!
//! "Already served" reads plants and fuel, not `city_is_powered`: the live
//! mirror does not import the host's powered flag, so that test reads every
//! fuel-powered city as dark.
//!
//! **Why the claim too.** Replays of the live turns changed no orders with the
//! credit alone: the scorer already hinted the plant (034948Z six turns from
//! t156, 105753Z five from t156), and none of those hints landed. A hint is
//! used only if the host's production blocker fires first; a finished city is
//! idle at the next export, the direct order answers it, and the Lua clears the
//! hint. Only 16% of 1,900 hints landed in the last 14 games. The direct pick
//! on an idle queue is the delegated governor (Shipyards, a Market, an Art
//! Museum), and the one claim that names a plant, `factory_claim_item`, names
//! Coal alone. So under the gene that claim takes the plant our fuel can run
//! ([`AdvancedAi::grid_plant_claim`]), and keeps it at the queue's head. With
//! the gene off, nothing changes.

use super::*;
use crate::game::City;
use crate::rules::BuildingSpec;

/// The turns a fuel stockpile is spread over when sizing what a plant can
/// feed: long enough that a banked 25 feeds one city, not five for a moment.
pub(crate) const PLANT_FUEL_HORIZON_TURNS: f64 = 20.0;

/// Each fuel, the effect naming it on its plant, and that plant.
const FUELLED_PLANTS: [(&str, &str, &str); 3] = [
    ("coal", "coal_per_power", "coal_power_plant"),
    ("oil", "oil_per_power", "oil_power_plant"),
    ("uranium", "uranium_per_power", "nuclear_power_plant"),
];

/// The tile `city`'s standing, unpillaged Industrial Zone holds.
fn zone_site(g: &Game, city: &City) -> Option<Pos> {
    city.districts.iter().find_map(|(district, pos)| {
        (g.district_family(*district) == crate::name!("industrial_zone")
            && g.map.get(*pos).is_some_and(|tile| !tile.pillaged))
        .then_some(*pos)
    })
}

/// Fuel a plant can count on each turn.
fn fuel_each_turn(g: &Game, pid: usize, fuel: &str) -> f64 {
    g.strategic_resource_rate(pid, fuel).max(0.0)
        + g.strategic_stockpile(pid, Name::new(fuel)) / PLANT_FUEL_HORIZON_TURNS
}

/// The Power `city` still needs after its renewables.
fn power_short(g: &Game, city: &City) -> f64 {
    (g.city_power_demand(city) - g.city_power_supply(city)).max(0.0)
}

/// The `powered_*` yields of the buildings `city` already holds.
fn powered_yields_held(g: &Game, city: &City) -> Yields {
    let mut held = Yields::default();
    for name in &city.buildings {
        let Some(building) = g.rules.buildings.get_interned(*name) else {
            continue;
        };
        // A regional building is powered by its own city's count.
        if building.regional_range > 0 || city.pillaged_buildings.contains(name) {
            continue;
        }
        let of = |key: &str| building.effects.get(key).copied().unwrap_or(0.0);
        held.food += of("powered_food");
        held.production += of("powered_production");
        held.gold += of("powered_gold");
        held.science += of("powered_science");
        held.culture += of("powered_culture");
        held.faith += of("powered_faith");
    }
    held
}

impl AdvancedAi {
    /// What the fuel-burning plant `spec` would switch on across the grid in
    /// reach of `city`'s Industrial Zone. Zero for anything else, or with no
    /// fuel to burn. See the module note.
    pub(super) fn plant_grid_yields(&self, g: &Game, city: &City, spec: &BuildingSpec) -> Yields {
        self.plant_grid_reach(g, city, spec).0
    }

    /// The plant `cid` would claim under `plant-reads-the-grid` and the cities
    /// it would switch on: of the plants legal here, the one worth the most
    /// on the fuel we hold. `None` with the gene off, with no fuel, or with
    /// nothing to switch on, which leaves `factory_claim_item`'s Coal path.
    pub(super) fn grid_plant_claim(&self, g: &Game, pid: usize, cid: u32) -> Option<(Item, usize)> {
        if !self.plant_reads_the_grid {
            return None;
        }
        let city = g.cities.get(&cid)?;
        // The launch pad outranks a plant: replayed 034948Z t160 put the plant
        // ahead of a Spaceport with no progress banked.
        if let Some(Item::District { district, .. }) = city.queue.first() {
            if g.district_family(*district) == crate::name!("spaceport") {
                return None;
            }
        }
        // One plant at a time: only a standing plant counts as serving a city,
        // so a second city would claim the same grid while the first builds.
        let queued = g.cities.values().any(|other| {
            other.owner == pid
                && other.queue.iter().any(|item| {
                    matches!(item, Item::Building { building }
                        if FUELLED_PLANTS.iter().any(|(_, _, plant)| building.as_str() == *plant))
                })
        });
        if queued {
            return None;
        }
        FUELLED_PLANTS
            .iter()
            .filter_map(|(_, _, plant)| {
                let item = BasicAi::civ_building(g, pid, cid, plant)?;
                let spec = match &item {
                    Item::Building { building } => g.rules.buildings.get(building)?,
                    _ => return None,
                };
                let (yields, reach) = self.plant_grid_reach(g, city, spec);
                (reach > 0).then_some((item, reach, yields.total()))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(item, reach, _)| (item, reach))
    }

    /// What `spec` would switch on, and in how many cities.
    fn plant_grid_reach(&self, g: &Game, city: &City, spec: &BuildingSpec) -> (Yields, usize) {
        let nothing = (Yields::default(), 0);
        let per_unit = spec.effects.get("power_generated").copied().unwrap_or(0.0);
        if per_unit <= 0.0 {
            return nothing;
        }
        let Some(&(fuel, _, plant)) = FUELLED_PLANTS
            .iter()
            .find(|(_, key, _)| spec.effects.get(*key).copied().unwrap_or(0.0) > 0.0)
        else {
            return nothing;
        };
        if self.diplomatic_lane_refuses_fossil_power(plant) {
            return nothing;
        }
        let Some(origin) = zone_site(g, city) else {
            return nothing;
        };
        let pid = city.owner;
        let mut ours: Vec<&City> = g.cities.values().filter(|c| c.owner == pid).collect();
        ours.sort_by_key(|c| c.id);
        // Plants already burning: their zone, and the fuel they burn.
        let burning: Vec<(Pos, &str)> = FUELLED_PLANTS
            .iter()
            .filter(|(burnt, _, _)| fuel_each_turn(g, pid, burnt) > 0.0)
            .flat_map(|(burnt, _, built)| {
                let built = Name::new(built);
                ours.iter()
                    .filter(move |c| {
                        c.buildings.contains(&built) && !c.pillaged_buildings.contains(&built)
                    })
                    .filter_map(|c| zone_site(g, c))
                    .map(|site| (site, *burnt))
                    .collect::<Vec<_>>()
            })
            .collect();
        let serves = |site: Pos, other: &City| g.wdist(site, other.pos) <= spec.regional_range;
        let units = |other: &City| (power_short(g, other) / per_unit).ceil();
        let mut budget = fuel_each_turn(g, pid, fuel);
        for other in ours.iter().copied() {
            if burning
                .iter()
                .any(|(site, burnt)| *burnt == fuel && serves(*site, other))
            {
                budget -= units(other);
            }
        }
        let mut switched = Yields::default();
        let mut reach = 0;
        for other in ours.iter().copied() {
            if !serves(origin, other) || burning.iter().any(|(site, _)| serves(*site, other)) {
                continue;
            }
            let need = units(other);
            let yields = powered_yields_held(g, other);
            if need <= 0.0 || yields.total() <= 0.0 || budget < need {
                continue;
            }
            budget -= need;
            switched.add(yields);
            reach += 1;
        }
        (switched, reach)
    }
}

#[cfg(test)]
#[path = "plant_reads_the_grid/tests.rs"]
mod tests;
