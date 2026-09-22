//! Native boards do not run the simulator's era-trigger accumulator. Recover
//! the observable research and construction channels before an era choice.
//! These are lower bounds, not invented counts for conversions, trade-route
//! completions, combat, or exploration events that these snapshots cannot prove.
use super::civvis_node_name;
use crate::rules::Rules;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
struct Observation {
    kind: String,
    #[serde(default)]
    run: String,
    turn: u32,
    world_era: Option<i64>,
    boosted_techs: Option<BTreeSet<String>>,
    boosted_civics: Option<BTreeSet<String>>,
    cities: Option<Vec<City>>,
}

#[derive(Deserialize)]
struct City {
    id: i64,
    buildings: Option<BTreeSet<String>>,
    districts: Option<Vec<District>>,
}

#[derive(Deserialize)]
struct District {
    #[serde(rename = "type")]
    kind: String,
    x: i32,
    y: i32,
    complete: Option<bool>,
}

type Counts = BTreeMap<String, i64>;

fn add(counts: &mut Counts, trigger: &str, amount: usize) {
    if amount > 0 {
        *counts.entry(trigger.into()).or_default() += amount as i64;
    }
}

fn building_credit(counts: &mut Counts, rules: &Rules, native: &str) {
    let Some(name) = civvis_node_name(&rules.buildings, native, "BUILDING_") else {
        return;
    };
    let spec = &rules.buildings[name.as_str()];
    if spec.wonder {
        return;
    }
    add(
        counts,
        "culture_building",
        usize::from(spec.yields.culture > 0.0),
    );
    add(
        counts,
        "science_building",
        usize::from(spec.yields.science > 0.0),
    );
    let era = spec
        .tech
        .and_then(|tech| rules.techs.get(&tech))
        .map(|tech| tech.era)
        .or_else(|| {
            spec.civic
                .and_then(|civic| rules.civics.get(&civic))
                .map(|civic| civic.era)
        })
        .unwrap_or(0);
    add(counts, "industrial_building", usize::from(era >= 4));
}

/// Read no future turn. Within-turn repeats are sets, not additional triggers.
/// At an era boundary retain only activity witnessed before the transition;
/// changes first seen in the boundary snapshot have ambiguous timing and do not
/// inflate the preceding era. A reload's initial inventory is only a baseline.
pub(super) fn through(raw: &str, turn: u32) -> Option<Counts> {
    let rules = Rules::embedded();
    let mut previous: Option<Observation> = None;
    let mut current = Counts::new();
    let mut last = None;
    let mut techs = BTreeSet::new();
    let mut civics = BTreeSet::new();
    let mut buildings = BTreeSet::new();
    let mut districts = BTreeSet::new();
    for line in raw.lines().filter(|line| line.contains("\"state\"")) {
        let Ok(observation) = serde_json::from_str::<Observation>(line) else {
            continue;
        };
        if observation.kind != "state" || observation.turn > turn {
            continue;
        }
        let Some(era) = observation.world_era.filter(|era| *era >= 0) else {
            continue;
        };
        let reset = previous.as_ref().is_some_and(|old| {
            old.run != observation.run
                || observation.turn < old.turn
                || old.world_era.is_some_and(|old_era| era < old_era)
        });
        if reset {
            previous = None;
            current.clear();
            last = None;
            techs.clear();
            civics.clear();
            buildings.clear();
            districts.clear();
        }
        let same_era = previous
            .as_ref()
            .is_some_and(|old| old.world_era == Some(era));
        if let Some(old) = &previous {
            if old.world_era != Some(era) {
                last = (old.world_era == Some(era - 1)).then(|| std::mem::take(&mut current));
                current.clear();
            }
        }
        for (observed, seen, trigger) in [
            (&observation.boosted_techs, &mut techs, "eureka"),
            (&observation.boosted_civics, &mut civics, "inspiration"),
        ] {
            if let Some(observed) = observed {
                let known = previous.as_ref().is_some_and(|old| match trigger {
                    "eureka" => old.boosted_techs.is_some(),
                    _ => old.boosted_civics.is_some(),
                });
                if same_era && known {
                    add(&mut current, trigger, observed.difference(seen).count());
                }
                seen.extend(observed.iter().cloned());
            }
        }
        if let Some(cities) = &observation.cities {
            for city in cities {
                let old_city = previous
                    .as_ref()
                    .and_then(|old| old.cities.as_ref())
                    .and_then(|old| old.iter().find(|old_city| old_city.id == city.id));
                if let Some(built) = &city.buildings {
                    for building in built {
                        if buildings.insert((city.id, building.clone()))
                            && same_era
                            && old_city.is_some_and(|old| old.buildings.is_some())
                        {
                            building_credit(&mut current, &rules, building);
                        }
                    }
                }
                if let Some(placed) = &city.districts {
                    for district in placed
                        .iter()
                        .filter(|district| district.complete == Some(true))
                    {
                        if districts.insert((
                            city.id,
                            district.kind.clone(),
                            district.x,
                            district.y,
                        )) && same_era
                            && old_city
                                .and_then(|old| old.districts.as_ref())
                                .is_some_and(|old| {
                                    old.iter()
                                        .find(|entry| {
                                            entry.kind == district.kind
                                                && entry.x == district.x
                                                && entry.y == district.y
                                        })
                                        .is_none_or(|entry| entry.complete == Some(false))
                                })
                            && !matches!(
                                district.kind.as_str(),
                                "DISTRICT_CITY_CENTER" | "DISTRICT_WONDER"
                            )
                        {
                            add(&mut current, "district", 1);
                        }
                    }
                }
            }
        }
        previous = Some(observation);
    }
    last
}

#[cfg(test)]
mod tests;
