use super::*;
use crate::name::Name;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn board() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 32, 22, 610_038_860, 250, 0, false);
    g.game_speed = GameSpeed::Online;
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.hills = true;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.pillaged = false;
    }
    let first = g.found_city_for(0, (5, 5), None);
    let second = g.found_city_for(0, (19, 5), None);
    for cid in [first, second] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.queue.clear();
        city.pop = 3;
        let worked = city
            .owned_tiles
            .iter()
            .copied()
            .filter(|pos| *pos != city.pos)
            .take(3)
            .collect();
        Arc::make_mut(&mut g.observed_city_worked_tiles).insert(cid, worked);
        g.spawn_test_unit("warrior", 0, g.cities[&cid].pos);
    }
    let builder = g.spawn_test_unit("builder", 0, g.cities[&first].pos);
    g.players[0].techs.insert(crate::name!("mining"));
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 10.0;
    g.turn = 40;
    g.current = 0;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 40,
        rush: false,
    };
    (
        g,
        AdvancedAi::targeting(super::super::VictoryTarget::Domination),
        plan,
        second,
        builder,
    )
}

#[test]
fn one_empire_builder_does_not_suppress_profitable_work_in_another_city() {
    let (mut g, ai, plan, city, _) = board();
    assert_eq!(ai.counts(&g, 0).builders, 1);
    let (selected, item) = ai.named_productive_workforce_target(&g, 0, &plan).unwrap();
    assert_eq!(selected, city);
    assert!(matches!(item, Item::Unit { unit } if unit == "builder"));
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&city].queue.first(), Some(&item));
    assert!(
        ai.named_productive_workforce_target(&g, 0, &plan).is_none(),
        "bounded workforce includes the new queue"
    );
}

#[test]
fn a_nearby_charged_builder_already_services_the_local_jobs() {
    let (mut g, ai, plan, city, builder) = board();
    // Keep the first city committed while its worker moves to the second.
    let first = g.map.tiles[&g.units[&builder].pos].owner_city.unwrap();
    g.cities.get_mut(&first).unwrap().queue = vec![Item::Building {
        building: crate::name!("monument"),
    }];
    g.units.get_mut(&builder).unwrap().pos = g.cities[&city].pos;
    assert!(ai.named_productive_workforce_target(&g, 0, &plan).is_none());
}

#[test]
fn unworked_jobs_and_jobs_without_time_to_repay_do_not_justify_a_new_unit() {
    let (mut g, ai, plan, city, _) = board();
    let worked = g.observed_city_worked_tiles[&city].clone();
    Arc::make_mut(&mut g.observed_city_worked_tiles).insert(city, vec![worked[0]]);
    assert!(ai.named_productive_workforce_target(&g, 0, &plan).is_some());
    g.turn = 240;
    assert!(ai.named_productive_workforce_target(&g, 0, &plan).is_none());
    g.turn = 40;
    Arc::make_mut(&mut g.observed_city_worked_tiles).insert(city, Vec::new());
    assert!(ai.named_productive_workforce_target(&g, 0, &plan).is_none());
}

#[test]
fn reservations_preserve_occupied_threatened_and_bankrupt_queues() {
    let (g, ai, plan, city, _) = board();
    let mut occupied = g.clone();
    occupied.cities.get_mut(&city).unwrap().queue = vec![Item::Building {
        building: crate::name!("monument"),
    }];
    assert!(ai
        .named_productive_workforce_target(&occupied, 0, &plan)
        .is_none());
    let mut threatened = plan.clone();
    threatened.threatened_city = Some(city);
    assert!(ai
        .named_productive_workforce_target(&g, 0, &threatened)
        .is_none());
    let mut recovery = plan.clone();
    recovery.strategy = GrandStrategy::Recovery;
    assert!(ai
        .named_productive_workforce_target(&g, 0, &recovery)
        .is_none());
    let mut attacked = g.clone();
    attacked.cities.get_mut(&city).unwrap().last_attacked = g.turn;
    assert!(ai
        .named_productive_workforce_target(&attacked, 0, &plan)
        .is_none());
    let mut bankrupt = g.clone();
    bankrupt.players[0].gold = 0.0;
    bankrupt.players[0].gold_per_turn = -5.0;
    bankrupt.at_war.insert((0, 1));
    assert!(ai
        .named_productive_workforce_target(&bankrupt, 0, &plan)
        .is_none());
}

#[test]
fn enough_time_to_repay_and_the_local_defense_floor_are_required() {
    let (mut g, ai, plan, _, _) = board();
    g.turn = 245;
    assert!(ai.named_productive_workforce_target(&g, 0, &plan).is_none());
    g.turn = 40;
    let warrior = g.units.values().find(|u| u.kind == "warrior").unwrap().id;
    g.remove_unit(warrior);
    assert!(ai.named_productive_workforce_target(&g, 0, &plan).is_none());
}

#[test]
fn actual_tile_ownership_and_legality_are_required() {
    let (mut g, ai, plan, city, _) = board();
    for pos in g.observed_city_worked_tiles[&city].clone() {
        g.map.tiles.get_mut(&pos).unwrap().owner_city = None;
    }
    assert!(ai.named_productive_workforce_target(&g, 0, &plan).is_none());
    let (mut g, ai, plan, city, _) = board();
    g.blocked_improvement_sites = Arc::new(
        g.observed_city_worked_tiles[&city]
            .iter()
            .copied()
            .collect(),
    );
    assert!(ai.named_productive_workforce_target(&g, 0, &plan).is_none());
}

#[test]
fn adaptive_genomes_and_science_launch_cities_keep_their_contracts() {
    let (mut g, _, plan, city, _) = board();
    assert!(AdvancedAi::new()
        .named_productive_workforce_target(&g, 0, &plan)
        .is_none());
    crate::game::install_test_district(&mut g, city, "spaceport");
    let ai = AdvancedAi::targeting(super::super::VictoryTarget::Science);
    assert!(ai.named_productive_workforce_target(&g, 0, &plan).is_none());
    assert!(g.cities[&city]
        .districts
        .contains_key(&Name::new("spaceport")));
}

#[test]
fn researched_improvement_forecast_matches_the_completed_operation() {
    let (mut g, _, _, city, _) = board();
    let pos = g.observed_city_worked_tiles[&city][0];
    g.players[0].techs.insert(crate::name!("apprenticeship"));
    let before = g.modeled_tile_yields(pos);
    let gain = g.improvement_yield_change(0, pos, crate::name!("mine"));
    assert!(gain.production > g.rules.improvements["mine"].yields.production);
    assert!(g.map.tiles[&pos].improvement.is_none());
    let builder = g.spawn_test_unit("builder", 0, pos);
    g.apply(
        0,
        &Action::Improve {
            unit: builder,
            improvement: crate::name!("mine"),
        },
    )
    .unwrap();
    assert!(
        (g.modeled_tile_yields(pos).production - before.production - gain.production).abs() < 1e-9
    );
}

#[test]
fn a_slow_builder_queue_elsewhere_does_not_block_a_profitable_local_worker() {
    let (mut g, ai, plan, city, builder) = board();
    let first = g.map.tiles[&g.units[&builder].pos].owner_city.unwrap();
    g.remove_unit(builder);
    g.cities.get_mut(&first).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("builder"),
    }];
    assert_eq!(
        ai.named_productive_workforce_target(&g, 0, &plan)
            .unwrap()
            .0,
        city
    );
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert!(ai.named_productive_workforce_target(&g, 0, &plan).is_none());
}
