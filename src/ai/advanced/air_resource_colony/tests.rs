use super::super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::Pos;
use crate::game::{Game, Item};
use crate::name::Name;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, Pos, u32, u32) {
    let mut g = Game::new_full(3, 40, 24, 379_500, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = true;
    }
    let home = g.found_city_for(0, (8, 10), None);
    g.found_city_for(0, (4, 10), None);
    g.found_city_for(1, (30, 10), None);
    g.players[2].is_minor = true;
    g.found_city_for(2, (18, 10), None);
    g.players[0].envoys.push((2, 6));
    g.players[0].techs.extend(
        [
            "mining",
            "industrialization",
            "flight",
            "radio",
            "advanced_flight",
        ]
        .into_iter()
        .map(Name::new),
    );
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    crate::game::install_test_district(&mut g, home, "aerodrome");
    for city in g.cities.values_mut().filter(|city| city.owner == 0) {
        city.pop = 8;
        city.queue.clear();
    }
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = 20.0;
    g.turn = 180;
    g.current = 0;
    let deposit = (12, 10);
    g.map.tiles.get_mut(&deposit).unwrap().resource = Some(crate::name!("aluminum"));
    let guard = g.spawn_test_unit("musketman", 0, (12, 11));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.frontier_loyalty = true;
    ai.base.book_pos = 4;
    ai.last_city_count = 2;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.plan = Some(plan.clone());
    (g, ai, plan, deposit, home, guard)
}

#[test]
fn an_actual_domination_turn_requests_one_supply_colony_at_the_city_target() {
    let (mut g, mut ai, plan, deposit, home, _) = fixture();
    let item = Item::Unit {
        unit: crate::name!("settler"),
    };
    assert!(g.can_produce(0, home, &item));
    assert!(
        ai.air_resource_settlement_sites(&g, 0, &std::collections::BTreeSet::from([deposit]))
            .contains(&deposit)
    );
    assert!(
        ai.production_value(&g, 0, home, &item, &plan, &ai.counts(&g, 0)) < 0.0,
        "ordinary expansion is already at its city target"
    );
    ai.plan_observed_turn(&mut g, 0);
    let queued = g
        .player_city_ids(0)
        .into_iter()
        .filter(|cid| g.cities[cid].queue.first() == Some(&item))
        .count();
    assert_eq!(
        queued, 1,
        "the missing Aluminum city gets one idle queue before ordinary production"
    );
}

#[test]
fn a_new_untargeted_settler_chooses_the_guarded_resource_colony() {
    let (mut g, mut ai, _, deposit, home, _) = fixture();
    let settler = g.spawn_test_unit("settler", 0, g.cities[&home].pos);
    ai.advanced_settler_step(&mut g, 0, settler);
    assert_eq!(
        ai.settler_target(settler),
        Some(deposit),
        "acquisition must carry through to the new walker's actual target"
    );
}

#[test]
fn colony_acquisition_preserves_existing_queues_and_in_flight_settlers() {
    let (mut g, ai, plan, _, home, _) = fixture();
    let protected = Item::Building {
        building: crate::name!("monument"),
    };
    g.cities
        .get_mut(&home)
        .unwrap()
        .queue
        .push(protected.clone());
    let other = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| *cid != home)
        .unwrap();
    assert!(ai.reserve_air_resource_colony(&mut g, 0, &plan));
    assert_eq!(g.cities[&home].queue, vec![protected]);
    assert_eq!(
        g.cities[&other].queue,
        vec![Item::Unit {
            unit: crate::name!("settler")
        }]
    );
    assert!(
        !ai.reserve_air_resource_colony(&mut g, 0, &plan),
        "one queued walker closes the request"
    );

    let (mut g, ai, plan, _, home, _) = fixture();
    g.spawn_test_unit("settler", 0, g.cities[&home].pos);
    assert!(
        !ai.reserve_air_resource_colony(&mut g, 0, &plan),
        "reuse the existing walker first"
    );
}

#[test]
fn a_valid_ordinary_target_keeps_its_walker() {
    let (mut g, mut ai, _, deposit, home, _) = fixture();
    let settler = g.spawn_test_unit("settler", 0, g.cities[&home].pos);
    let ordinary = (8, 5);
    ai.settler_targets.insert(settler, ordinary);
    ai.advanced_settler_step(&mut g, 0, settler);
    assert_eq!(ai.settler_target(settler), Some(ordinary));
    assert_ne!(ai.settler_target(settler), Some(deposit));
}

#[test]
fn supply_request_keeps_knowledge_security_legality_and_clock_guards() {
    for case in [
        "off",
        "science",
        "field",
        "unrevealed",
        "uncharted",
        "frontier",
        "no_guard",
        "negative_loyalty",
        "minor",
        "sufficient",
        "connection",
        "suzerain_connection",
        "occupied",
        "blocked",
        "population",
        "recent_attack",
        "clock",
        "queue",
        "threat",
        "host_menu",
    ] {
        let (mut g, mut ai, mut plan, deposit, home, guard) = fixture();
        match case {
            "off" => ai.disable_air_surge_2(),
            "science" => {
                ai.victory_target = Some(VictoryTarget::Science);
            }
            "field" => {
                g.cities.get_mut(&home).unwrap().districts.clear();
            }
            "unrevealed" => {
                g.players[0].techs.remove(&crate::name!("radio"));
            }
            "uncharted" => {
                g.players[0].explored.remove(&deposit);
            }
            "frontier" => {
                let sites = g.player_city_ids(0);
                for cid in sites {
                    g.cities.get_mut(&cid).unwrap().pos = (2, 2);
                }
                g.players[0].explored.remove(&(12, 14));
                assert!(AdvancedAi::beyond_loyalty_reach(&g, 0, deposit));
            }
            "no_guard" => {
                g.remove_unit(guard);
            }
            "negative_loyalty" => {
                let rival = g.found_city_for(1, (14, 14), None);
                g.cities.get_mut(&rival).unwrap().pop = 40;
                let mut forecast = g.speculative_clone();
                let city = forecast.found_city_for(0, deposit, None);
                assert!(forecast.city_loyalty_per_turn(&forecast.cities[&city]) < 0.0);
            }
            "minor" => g.players[0].envoys.clear(),
            "sufficient" => {
                g.map.tiles.get_mut(&g.cities[&home].pos).unwrap().resource =
                    Some(crate::name!("aluminum"));
            }
            "connection" | "suzerain_connection" => {
                let city = if case == "connection" {
                    home
                } else {
                    g.player_city_ids(2)[0]
                };
                let site = g.cities[&city]
                    .owned_tiles
                    .iter()
                    .copied()
                    .find(|site| {
                        *site != g.cities[&city].pos && g.map.tiles[site].district.is_none()
                    })
                    .unwrap();
                g.map.tiles.get_mut(&site).unwrap().resource = Some(crate::name!("aluminum"));
                g.players[0].explored.insert(site);
                assert!(
                    g.valid_improvements(0, site)
                        .contains(&crate::name!("mine")),
                    "{case}"
                );
            }
            "occupied" => {
                g.map.tiles.get_mut(&deposit).unwrap().owner_city = Some(home);
            }
            "blocked" => {
                std::sync::Arc::make_mut(&mut g.blocked_city_sites).insert(deposit);
            }
            "population" => {
                for city in g.cities.values_mut().filter(|city| city.owner == 0) {
                    city.pop = 1;
                }
            }
            "recent_attack" => {
                for city in g.cities.values_mut().filter(|city| city.owner == 0) {
                    city.last_attacked = g.turn;
                }
            }
            "clock" => {
                g.turn = 249;
                plan.assessed_turn = g.turn;
            }
            "queue" => {
                for city in g.cities.values_mut().filter(|city| city.owner == 0) {
                    city.queue.push(Item::Unit {
                        unit: crate::name!("builder"),
                    });
                }
            }
            "threat" => plan.threatened_city = Some(home),
            "host_menu" => {
                for cid in g.player_city_ids(0) {
                    std::sync::Arc::make_mut(&mut g.host_buildable).insert(cid, Default::default());
                }
            }
            _ => unreachable!(),
        }
        assert!(!ai.reserve_air_resource_colony(&mut g, 0, &plan), "{case}");
    }
}

#[test]
fn a_supply_target_losing_its_defender_is_retired_before_founding() {
    // Use ground outside the city-state buffer: the new appointment must
    // retain its own permission, even where ordinary economic colonies are
    // allowed without that exception.
    let (mut g, mut ai, _, _, home, guard) = fixture();
    let deposit = (8, 16);
    g.map.tiles.get_mut(&(12, 10)).unwrap().resource = None;
    g.map.tiles.get_mut(&deposit).unwrap().resource = Some(crate::name!("aluminum"));
    g.units.get_mut(&guard).unwrap().pos = (8, 17);
    let settler = g.spawn_test_unit("settler", 0, g.cities[&home].pos);
    ai.advanced_settler_step(&mut g, 0, settler);
    assert_eq!(ai.air_resource_colony_target, Some((settler, deposit)));
    g.units.get_mut(&settler).unwrap().pos = deposit;
    g.units.get_mut(&settler).unwrap().moves_left = 2.0;
    g.remove_unit(guard);
    ai.advanced_settler_step(&mut g, 0, settler);
    assert!(g.city_at(deposit).is_none());
    assert!(ai.settler_site_is_dead(settler, deposit));
    assert_ne!(ai.settler_target(settler), Some(deposit));
}

#[test]
fn a_supply_appointment_survives_native_unit_id_remapping() {
    let (_, mut ai, _, deposit, _, _) = fixture();
    ai.air_resource_colony_target = Some((17, deposit));
    ai.settler_targets.insert(17, deposit);
    ai.remap_unit_memory(&std::collections::BTreeMap::from([(17, 42)]));
    assert_eq!(ai.air_resource_colony_target, Some((42, deposit)));
    assert_eq!(ai.settler_target(42), Some(deposit));
    ai.remap_unit_memory(&std::collections::BTreeMap::new());
    assert_eq!(ai.air_resource_colony_target, None);
}

#[test]
fn the_peacetime_force_gap_leaves_one_supply_queue() {
    let (mut g, mut ai, plan, _, _, _) = fixture();
    g.record_contact(0, 1);
    for pos in [(30, 10), (30, 11), (31, 10), (29, 11)] {
        g.spawn_test_unit("musketman", 1, pos);
    }
    ai.peacetime_deterrence = true;
    ai.army_target_weighs_the_enemy = true;
    assert!(ai.peacetime_deterrence_force_gap(&g, 0, &plan).is_some());
    ai.plan_observed_turn(&mut g, 0);
    let item = Item::Unit {
        unit: crate::name!("settler"),
    };
    assert_eq!(
        g.player_city_ids(0)
            .into_iter()
            .filter(|cid| g.cities[cid].queue.first() == Some(&item))
            .count(),
        1
    );
}

#[test]
fn deferred_production_uses_the_same_guarded_supply_request() {
    let (mut g, ai, _, _, home, _) = fixture();
    for cid in g.player_city_ids(0).into_iter().filter(|cid| *cid != home) {
        g.cities.get_mut(&cid).unwrap().queue.push(Item::Unit {
            unit: crate::name!("builder"),
        });
    }
    assert_eq!(
        ai.preview_live_production(&g, 0, home),
        Some(Item::Unit {
            unit: crate::name!("settler")
        })
    );
    assert!(
        g.cities[&home].queue.is_empty(),
        "the preview is disposable"
    );
}

#[test]
fn stalled_founding_keeps_the_tracked_supply_permission() {
    let (mut g, mut ai, _, _, _, guard) = fixture();
    let deposit = (8, 16);
    g.map.tiles.get_mut(&(12, 10)).unwrap().resource = None;
    g.map.tiles.get_mut(&deposit).unwrap().resource = Some(crate::name!("aluminum"));
    g.units.get_mut(&guard).unwrap().pos = (8, 17);
    let settler = g.spawn_test_unit("settler", 0, deposit);
    ai.air_resource_colony_target = Some((settler, deposit));
    ai.enable_settler_founds_when_stalled();
    assert!(!ai.air_resource_colony_target_refused(&g, 0, settler, deposit));
    g.remove_unit(guard);
    assert!(!ai.founds_where_it_stands(&mut g, 0, settler, deposit));
    assert!(g.city_at(deposit).is_none());
}

fn next_own_turn(g: &mut Game) {
    loop {
        g.apply(g.current, &crate::game::Action::EndTurn).unwrap();
        if g.current == 0 {
            break;
        }
    }
}

#[test]
fn a_produced_settler_can_walk_and_connect_the_city_center_resource() {
    let (mut g, mut ai, plan, deposit, _, _) = fixture();
    assert!(ai.reserve_air_resource_colony(&mut g, 0, &plan));
    let mut settler = None;
    for _ in 0..40 {
        next_own_turn(&mut g);
        settler = g
            .units
            .values()
            .find(|unit| unit.owner == 0 && unit.kind == "settler")
            .map(|unit| unit.id);
        if settler.is_some() {
            break;
        }
    }
    let settler = settler.expect("normal production completes the requested Settler");
    ai.advanced_settler_step(&mut g, 0, settler);
    assert_eq!(ai.settler_target(settler), Some(deposit));
    // Exercise legal model movement rather than teleporting an arrival.
    // Other units are held stationary in this controlled supply scenario;
    // this is not a native-game or whole-controller strength claim.
    for _ in 0..20 {
        if g.units[&settler].pos == deposit {
            break;
        }
        let step = g.route_step(settler, deposit, 0).unwrap();
        if g.can_move(settler, step) {
            g.apply(
                0,
                &crate::game::Action::Move {
                    unit: settler,
                    to: step,
                },
            )
            .unwrap();
        } else {
            next_own_turn(&mut g);
        }
    }
    assert_eq!(g.units[&settler].pos, deposit);
    if g.units[&settler].moves_left <= 0.0 {
        next_own_turn(&mut g);
    }
    ai.advanced_settler_step(&mut g, 0, settler);
    let city = g
        .city_at(deposit)
        .expect("normal guarded arrival founds the colony");
    assert_eq!(g.cities[&city].owner, 0);
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 2.0);
}

#[test]
fn an_expired_route_deferral_cannot_permanently_block_a_replacement() {
    let (mut g, mut ai, plan, deposit, _, _) = fixture();
    ai.settler_threat_deferrals.insert(deposit, g.turn + 1);
    assert!(!ai.reserve_air_resource_colony(&mut g, 0, &plan));
    ai.settler_threat_deferrals.insert(deposit, g.turn);
    assert!(
        ai.reserve_air_resource_colony(&mut g, 0, &plan),
        "no live Settler remains to prune its expired route deferral"
    );
}
