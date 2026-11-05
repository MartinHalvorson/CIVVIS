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
//! fuel-powered city as dark. With the gene off, nothing changes.

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
        let per_unit = spec.effects.get("power_generated").copied().unwrap_or(0.0);
        if per_unit <= 0.0 {
            return Yields::default();
        }
        let Some(&(fuel, _, plant)) = FUELLED_PLANTS
            .iter()
            .find(|(_, key, _)| spec.effects.get(*key).copied().unwrap_or(0.0) > 0.0)
        else {
            return Yields::default();
        };
        if self.diplomatic_lane_refuses_fossil_power(plant) {
            return Yields::default();
        }
        let Some(origin) = zone_site(g, city) else {
            return Yields::default();
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
        }
        switched
    }
}

#[cfg(test)]
#[path = "plant_reads_the_grid/tests.rs"]
mod tests;
