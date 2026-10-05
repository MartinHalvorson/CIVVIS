use civvis::ai::{run_game_observed, AdvancedAi, Ai, VictoryTarget};
use civvis::game::{Game, GameOptions};
use std::collections::BTreeSet;

fn farm_opportunities(g: &Game) -> serde_json::Value {
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
            if !worked.contains(&destination) {
                continue;
            }
            let Some(city) = g.map.get(destination).and_then(|tile| tile.owner_city) else {
                continue;
            };
            if g.cities[&city].owner != 0 {
                continue;
            }
            let before_tile = g.workable_tile_yields(destination);
            attempted += 1;
            let before = g.city_yields(city);
            let before_worked = g.city_citizen_plan(city).worked_tiles;
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
            if trial
                .apply(
                    0,
                    &Action::Improve {
                        unit: builder,
                        improvement: civvis::name!("farm"),
                    },
                )
                .is_err()
            {
                continue;
            }
            let mut tile_delta = trial.workable_tile_yields(destination);
            tile_delta.add_scaled(before_tile, -1.0);
            if tile_delta.food <= 0.0 || tile_delta.production < 0.0 {
                continue;
            }
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
                .filter(|unit| unit.owner != 0 && g.rules.units[unit.kind].class == "military")
                .map(|unit| g.wdist(unit.pos, destination))
                .min();
            let known_hostile_reach: Vec<_> = g
                .units
                .values()
                .filter(|unit| {
                    unit.owner != 0
                        && g.rules.units[unit.kind].class == "military"
                        && (g.is_at_war(0, unit.owner) || g.players[unit.owner].is_barbarian)
                })
                .filter(|unit| g.threat_reach(unit.id).contains(&destination))
                .map(|unit| unit.id)
                .collect();
            let nearest_camp_in_input = g
                .barb_camps
                .keys()
                .map(|pos| g.wdist(*pos, destination))
                .min();
            opportunities.push(serde_json::json!({"builder":builder,"charges":worker.charges,
                    "builder_position":worker.pos,"destination":destination,"city":city,"population":c.pop,
                    "food_bank":c.food,"housing":g.city_housing(c),"amenities":g.city_amenity_surplus(c),
                    "raw_food_surplus":before.food-2.0*f64::from(c.pop),
                    "tile_delta":tile_delta,"city_before":before,"city_after":after,
                    "production_change":after.production-before.production,
                    "food_change":after.food-before.food,"science_change":after.science-before.science,
                    "before_worked":before_worked,"after_worked":after_worked,
                    "fresh_allowance_for_static_price":fresh_allowance_for_static_price,
                    "guard_at_destination_in_input":guard_present,
                    "nearest_foreign_military_distance_in_observation":nearest_foreign_military,
                    "known_native_hostile_reach":known_hostile_reach,"nearest_camp_record_distance":nearest_camp_in_input,
                    "scope":"Successful Farm on static native decision-board clone; not an executed job or future capture-safety proof"}));
        }
    }
    assert_eq!(
        original,
        serde_json::to_vec(&g).unwrap(),
        "observer mutated input board"
    );
    serde_json::json!({"turn":g.turn,"current":g.current,
        "attempted_worked_farm_tiles":attempted,"opportunities":opportunities,
        "original_board_unchanged":true,"scope":"Static model only; no improvement enacted"})
}

use std::io::Write;

fn drain_journal(journal: &civvis::reasoning::Journal, cursor: &mut u64, out: &mut std::fs::File) {
    let delta = journal.since(*cursor);
    writeln!(
        out,
        "{}",
        serde_json::json!({"type":"journal_window","previous_cursor":cursor,
            "next_cursor":delta.cursor,"dropped_total":delta.dropped,
            "truncated_turns_total":delta.truncated_turns,"reset":delta.reset})
    )
    .unwrap();
    for thought in delta.thoughts {
        writeln!(out, "{}", serde_json::json!({"type":"planning_thought","id":thought.id,"turn":thought.turn,
            "player":thought.player,"headline":thought.headline,"detail":thought.detail,"focus":thought.focus})).unwrap();
    }
    *cursor = delta.cursor;
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 5, "probe START_SEED GAMES DIFFICULTY ARM");
    let seed: u64 = args[1].parse().unwrap();
    let games: u64 = args[2].parse().unwrap();
    let difficulty = &args[3];
    assert!(matches!(difficulty.as_str(), "emperor" | "deity"));
    assert_eq!(
        args[4], "control",
        "coverage observer does not change decisions"
    );
    let arm = &args[4];
    println!("seed,turn,cities,production,rival_production,science,culture,gold,cumulative_production,alive,winner,population,granaries,housing_bound,housing_bound_without_granary,amenity_short_cities,builders,military_power,rival_cities,rival_population,builder_gold_buys,productive_improvements");
    for seed in seed..seed + games {
        let mut opts = GameOptions::new(4, 60, 38, seed, 150, 6);
        opts.map_script = civvis::setup::MapScript::Pangaea;
        opts.speed = "online".into();
        opts.difficulty = difficulty.into();
        opts.barbarian_difficulty = difficulty.into();
        opts.handicap_exempt = BTreeSet::from([0]);
        opts.civs = vec!["Gran Colombia".into()];
        opts.randomize_civs = true;
        let mut g = Game::new_with(opts);
        let mut ais = AdvancedAi::fleet(&g);
        ais[0] = AdvancedAi::targeting(VictoryTarget::Domination);
        ais[0].enable_live_bridge();
        assert_eq!(ais[0].productive_builder_escort_enabled(), false);
        assert!(
            !ais[0].city_strategy,
            "per-city priorities must be clear in this public baseline"
        );
        assert!(
            ais[0].wide_map_capacity,
            "live repair bundle must be enabled before play"
        );
        assert!(
            ais.iter().skip(1).all(|ai| !ai.wide_map_capacity),
            "stock rivals must retain their fixed configuration"
        );
        let setup = serde_json::json!({"seed":seed,"arm":arm,"difficulty":difficulty,"configuration":"public targeted live bridge, stock adaptive rivals, native simulation","wide_map_capacity":ais[0].wide_map_capacity,"weights_debug":format!("{:?}",ais[0].weights()),"handicaps":(0..4).map(|pid|g.handicap_yield_pct(pid)).collect::<Vec<_>>(),"deployment_tags":civvis::ai::deployment_treatments(),"productive_builder_escort":ais[0].productive_builder_escort_enabled(),"coverage_only":true,"weights_vec":ais[0].weights().to_vec(),"actual_Firaxis_verification":false});
        std::fs::write(
            format!(
                "/tmp/civvis-production-food-builder-coverage-{difficulty}-{seed}-{arm}-setup.json"
            ),
            serde_json::to_vec_pretty(&setup).unwrap(),
        )
        .unwrap();
        let mut detail = std::fs::File::create(format!(
            "/tmp/civvis-production-food-builder-coverage-{difficulty}-{seed}-{arm}-cities.jsonl"
        ))
        .unwrap();
        let mut route_detail = std::fs::File::create(format!(
            "/tmp/civvis-production-food-builder-coverage-{difficulty}-{seed}-{arm}-routes.jsonl"
        ))
        .unwrap();
        let mut farm_detail = std::fs::File::create(format!(
            "/tmp/civvis-production-food-builder-coverage-{difficulty}-{seed}-{arm}-farms.jsonl"
        ))
        .unwrap();
        let journal = civvis::reasoning::Journal::recording();
        ais[0].attach_journal(journal.handle());
        let mut cursor = 0;
        let mut thoughts = std::fs::File::create(format!(
            "/tmp/civvis-production-food-builder-coverage-{difficulty}-{seed}-{arm}-thoughts.jsonl"
        ))
        .unwrap();
        let mut frames = std::fs::File::create(format!(
            "/tmp/civvis-production-food-builder-coverage-{difficulty}-{seed}-{arm}-frames.jsonl"
        ))
        .unwrap();
        let mut log_cursor = 0;
        let mut sum = 0.0;
        run_game_observed(&mut g, &mut ais, |g| {
            drain_journal(&journal, &mut cursor, &mut thoughts);
            let _memo = g.query_memo();
            let cities = g.player_city_ids(0);
            let p: f64 = cities.iter().map(|c| g.city_yields(*c).production).sum();
            sum += p;
            let focal_actions: Vec<_> = g
                .log
                .iter()
                .skip(log_cursor)
                .filter(|(pid, _)| *pid == 0)
                .collect();
            let builders: Vec<_> = g
                .player_unit_ids(0)
                .into_iter()
                .filter(|uid| g.units[uid].kind == "builder")
                .map(|uid| &g.units[&uid])
                .collect();
            let guards: Vec<_> = g
                .player_unit_ids(0)
                .into_iter()
                .filter(|uid| g.rules.units[g.units[uid].kind].class == "military")
                .map(|uid| &g.units[&uid])
                .collect();
            let city_summary: Vec<_> = cities.iter().map(|cid| { let c=&g.cities[cid]; serde_json::json!({"id":cid,"population":c.pop,"yields":g.city_yields(*cid),"queue":c.queue,"progress":c.production,"citizen_strategy":g.citizen_strategy(*cid),"directive":g.players[0].city_directives.get(cid)}) }).collect();
            writeln!(frames,"{}",serde_json::json!({"turn":g.turn,"actions":focal_actions,"builders":builders,"guards":guards,"cities":city_summary})).unwrap();
            let observed = g.player_decision_view(0);
            writeln!(farm_detail, "{}", farm_opportunities(&observed)).unwrap();
            let route_options: Vec<_> = g.legal_actions(0).into_iter().filter_map(|action| {
                let civvis::game::Action::TradeRoute { unit, city: destination } = action else { return None; };
                let origin = g.city_at(g.units[&unit].pos).expect("legal route has an origin");
                let city = &g.cities[&origin];
                Some(serde_json::json!({"unit":unit,"origin":origin,"destination":destination,
                    "domestic":g.cities[&destination].owner==0,"route_yields":g.trade_route_yields(0,destination),
                    "origin_population":city.pop,"origin_food_bank":city.food,
                    "origin_yields":g.city_yields(origin),"origin_housing":g.city_housing(city),
                    "origin_amenities":g.city_amenity_surplus(city),
                    "origin_raw_food_surplus":g.city_yields(origin).food-2.0*f64::from(city.pop)}))
            }).collect();
            let routes: Vec<_> = g.routes.iter().filter(|route|route.owner==0).map(|route| {
                serde_json::json!({"route":route,"domestic":g.cities.get(&route.dest).is_some_and(|city|city.owner==0),
                    "destination_model_yields":g.cities.get(&route.dest).map(|_|g.trade_route_yields(0,route.dest))})
            }).collect();
            writeln!(
                route_detail,
                "{}",
                serde_json::json!({"turn":g.turn,"current":g.current,
                "gold":g.players[0].gold,"income":g.players[0].gold_per_turn,
                "capacity":g.trade_capacity(0),"active":g.active_routes(0),
                "routes":routes,"available_options":route_options})
            )
            .unwrap();
            log_cursor = g.log.len();
            if [25, 50, 75].contains(&g.turn) {
                std::fs::write(
                    format!("/tmp/civvis-production-food-builder-coverage-{difficulty}-{seed}-{arm}-view-t{}.json",g.turn),
                    serde_json::to_vec(&g.player_decision_view(0)).unwrap(),
                ).unwrap();
            }
            if [25, 50, 75, 100, 125, 150].contains(&g.turn) {
                let city_records: Vec<_> = cities.iter().map(|cid| {
                    let c=&g.cities[cid];
                    serde_json::json!({"id":cid,"name":c.name,"population":c.pop,"food_bank":c.food,"housing":g.city_housing(c),"amenities":g.city_amenity_surplus(c),"yields":g.city_yields(*cid),"queue":c.queue,"progress":c.production,"buildings":c.buildings,"districts":c.districts,"worked_tiles":g.city_citizen_plan(*cid).worked_tiles})
                }).collect();
                let rival_cities: Vec<_> = (1..4).flat_map(|pid| {
                    g.player_city_ids(pid).into_iter().map(move |cid| (pid,cid))
                }).map(|(pid,cid)| {
                    let c = &g.cities[&cid];
                    serde_json::json!({"owner":pid,"id":cid,"name":c.name,"population":c.pop,
                        "housing":g.city_housing(c),"amenities":g.city_amenity_surplus(c),
                        "yields":g.city_yields(cid),"queue":c.queue,"buildings":c.buildings,
                        "districts":c.districts,"worked_tiles":g.city_citizen_plan(cid).worked_tiles})
                }).collect();
                writeln!(detail,"{}",serde_json::json!({"seed":seed,"turn":g.turn,"gold":g.players[0].gold,"income":g.players[0].gold_per_turn,"bankruptcy_amenity_penalty":g.players[0].bankruptcy_amenity_penalty,"government":g.players[0].government,"policies":g.players[0].policies,"cities":city_records,"rival_cities":rival_cities})).unwrap();
                let rival = (1..4)
                    .map(|pid| {
                        let cities = g.player_city_ids(pid);
                        let production: f64 =
                            cities.iter().map(|c| g.city_yields(*c).production).sum();
                        let population = cities.iter().map(|c| g.cities[c].pop as u64).sum::<u64>();
                        (production, cities.len(), population)
                    })
                    .max_by(|a, b| a.0.total_cmp(&b.0))
                    .unwrap();
                let science: f64 = cities.iter().map(|c| g.city_yields(*c).science).sum();
                let culture: f64 = cities.iter().map(|c| g.city_yields(*c).culture).sum();
                let granary = civvis::name!("granary");
                let bound = |cid: &&u32| {
                    let city = &g.cities[*cid];
                    g.city_housing(city) - city.pop as f64 <= 1.0
                };
                let builder_gold_buys = g.log.iter().filter(|(owner, action)| *owner == 0 && matches!(action, civvis::game::Action::Buy { unit, currency, .. } if unit == "builder" && currency == "gold")).count();
                let productive_improvements = g.log.iter().filter(|(owner, action)| *owner == 0 && matches!(action, civvis::game::Action::Improve { improvement, .. } if g.rules.improvements[improvement].yields.production > 0.0)).count();
                println!(
                    "{seed},{},{},{p:.6},{:.6},{science:.6},{culture:.6},{:.6},{sum:.6},{},,{},{},{},{},{},{},{:.6},{},{},{builder_gold_buys},{productive_improvements}",
                    g.turn,
                    cities.len(),
                    rival.0,
                    g.players[0].gold,
                    g.players[0].alive,
                    cities.iter().map(|c| g.cities[c].pop as u64).sum::<u64>(),
                    cities.iter().filter(|c| g.cities[c].buildings.contains(&granary)).count(),
                    cities.iter().filter(bound).count(),
                    cities.iter().filter(|c| bound(c) && !g.cities[c].buildings.contains(&granary)).count(),
                    cities.iter().filter(|c| g.city_amenity_surplus(&g.cities[c]) < 0).count(),
                    g.player_unit_ids(0).iter().filter(|u| g.units[u].kind == "builder").count(),
                    g.military_power(0),
                    rival.1,
                    rival.2
                );
            }
        });
        drain_journal(&journal, &mut cursor, &mut thoughts);
        let actions: Vec<_> = g.log.iter().collect();
        std::fs::write(
            format!(
                "/tmp/civvis-production-food-builder-coverage-{difficulty}-{seed}-{arm}-actions.json"
            ),
            serde_json::to_vec(&actions).unwrap(),
        )
        .unwrap();
        let save = serde_json::to_vec(&g).unwrap();
        std::fs::write(
            format!(
                "/tmp/civvis-production-food-builder-coverage-{difficulty}-{seed}-{arm}-final.json"
            ),
            save,
        )
        .unwrap();
        eprintln!(
            "{seed}: turn {}, winner {:?}, alive {}",
            g.turn, g.winner, g.players[0].alive
        );
    }
}
