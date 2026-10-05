use civvis::game::{Action, Game};
use std::collections::BTreeSet;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert!(!args.is_empty(), "probe DECISION_VIEW_JSON ...");
    for path in args {
        let bytes = std::fs::read(&path).unwrap();
        let g: Game = serde_json::from_slice(&bytes).unwrap();
        let original = serde_json::to_vec(&g).unwrap();
        let worked: BTreeSet<_> = g
            .player_city_ids(0)
            .into_iter()
            .flat_map(|city| g.city_citizen_plan(city).worked_tiles)
            .collect();
        let mut opportunities = Vec::new();
        let mut attempted = 0;
        for builder in g.player_unit_ids(0) {
            let worker = &g.units[&builder];
            if worker.kind != "builder" || worker.charges <= 0 || worker.moves_left <= 0.0 {
                continue;
            }
            let destinations: BTreeSet<_> = g
                .reachable(builder)
                .into_iter()
                .chain([worker.pos])
                .collect();
            for destination in destinations {
                let Some(city) = g.map.get(destination).and_then(|tile| tile.owner_city) else {
                    continue;
                };
                if g.cities[&city].owner != 0 {
                    continue;
                }
                let before_walk_tile = g.workable_tile_yields(destination);
                attempted += 1;
                let before_walk = g.city_yields(city);
                let before_walk_worked = g.city_citizen_plan(city).worked_tiles;
                let mut trial = g.speculative_clone();
                if worker.pos != destination {
                    if trial
                        .apply(
                            0,
                            &Action::MoveTo {
                                unit: builder,
                                to: destination,
                            },
                        )
                        .is_err()
                        || trial
                            .units
                            .get(&builder)
                            .is_none_or(|unit| unit.pos != destination)
                    {
                        continue;
                    }
                }
                let fresh_allowance_for_static_price = trial.units[&builder].moves_left <= 0.0;
                if fresh_allowance_for_static_price {
                    // This only prices the structural operation on a static
                    // copy. It does not simulate a next turn or enemy actions.
                    trial.units.get_mut(&builder).unwrap().moves_left = worker.moves_left;
                }
                let walked = trial;
                for improvement in walked.valid_improvements(0, destination) {
                    if !walked.rules.improvements[&improvement].builder_buildable {
                        continue;
                    }
                    let mut trial = walked.speculative_clone();
                    let before = trial.city_yields(city);
                    let before_worked = trial.city_citizen_plan(city).worked_tiles;
                    let before_tile = trial.workable_tile_yields(destination);
                    if trial
                        .apply(
                            0,
                            &Action::Improve {
                                unit: builder,
                                improvement,
                            },
                        )
                        .is_err()
                    {
                        continue;
                    }
                    let mut tile_delta = trial.workable_tile_yields(destination);
                    tile_delta.add_scaled(before_tile, -1.0);
                    let after = trial.city_yields(city);
                    let after_worked = trial.city_citizen_plan(city).worked_tiles;
                    let c = &g.cities[&city];
                    let guard_present = g.unit_ids_at(destination).iter().any(|uid| {
                        let unit = &g.units[uid];
                        unit.owner == 0 && g.rules.units[unit.kind].class == "military"
                    });
                    let nearest_foreign_military = g
                        .units
                        .values()
                        .filter(|unit| {
                            unit.owner != 0 && g.rules.units[unit.kind].class == "military"
                        })
                        .map(|unit| g.wdist(unit.pos, destination))
                        .min();
                    let hostile_reach: Vec<_> = g
                        .units
                        .values()
                        .filter(|u| {
                            u.owner != 0
                                && g.rules.units[u.kind].class == "military"
                                && (g.is_at_war(0, u.owner) || g.players[u.owner].is_barbarian)
                                && g.threat_reach(u.id).contains(&destination)
                        })
                        .map(|u| u.id)
                        .collect();
                    opportunities.push(serde_json::json!({"builder":builder,"charges":worker.charges,"improvement":improvement,
                    "worked_in_input":worked.contains(&destination),"resource":g.map.tiles[&destination].resource,
                    "standing_improvement":g.map.tiles[&destination].improvement,
                    "builder_position":worker.pos,"destination":destination,"city":city,"population":c.pop,
                    "food_bank":c.food,"housing":g.city_housing(c),"amenities":g.city_amenity_surplus(c),
                    "raw_food_surplus":before.food-2.0*f64::from(c.pop),
                    "tile_delta":tile_delta,"tile_before_walk":before_walk_tile,"city_before_walk":before_walk,"before_walk_worked":before_walk_worked,"city_before":before,"city_after":after,
                    "production_change":after.production-before.production,"known_native_hostile_reach":hostile_reach,
                    "food_change":after.food-before.food,"science_change":after.science-before.science,
                    "before_worked":before_worked,"after_worked":after_worked,
                    "fresh_allowance_for_static_price":fresh_allowance_for_static_price,
                    "guard_at_destination_in_input":guard_present,
                    "nearest_foreign_military_distance_in_observation":nearest_foreign_military,
                    "scope":"Legal structural improvement on static decision-board clone; after-walk baseline; not enacted work or safety proof"}));
                }
            }
        }
        assert_eq!(
            original,
            serde_json::to_vec(&g).unwrap(),
            "observer mutated input board"
        );
        println!(
            "{}",
            serde_json::json!({"source":path,"turn":g.turn,"current":g.current,
            "attempted_owned_destinations":attempted,"opportunities":opportunities,
            "original_board_unchanged":true,"new_games_played":0,"actual_Firaxis_verification":false})
        );
    }
}
