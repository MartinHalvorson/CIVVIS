use super::*;
use crate::{rules::Yields, Pos};
use std::sync::Arc;

fn board() -> (Game, u32, u32) {
    let mut g = Game::new(2, 24, 16, 71, 0, 0);
    g.victory_conditions = crate::game::VictoryConditions::parse("science").unwrap();
    let mut cities = Vec::new();
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.current = pid;
        g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
        cities.push(g.player_city_ids(pid)[0]);
    }
    g.current = 0;
    g.cities.get_mut(&cities[1]).unwrap().owner = 0;
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    g.players[0].civics = g.rules.civics.keys().copied().collect();
    g.players[0]
        .science_projects
        .extend(LAUNCHES.into_iter().map(str::to_owned));
    for &city in &cities {
        g.cities.get_mut(&city).unwrap().pop = 12;
        pad(&mut g, city);
        production(&mut g, city, 50.0);
    }
    (g, cities[0], cities[1])
}

fn pad(g: &mut Game, city: u32) -> Pos {
    let center = g.cities[&city].pos;
    let pos = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != center)
        .unwrap();
    let tile = g.map.tiles.get_mut(&pos).unwrap();
    tile.terrain = crate::name!("plains");
    tile.feature = None;
    tile.hills = false;
    tile.resource = None;
    tile.district = Some(crate::name!("spaceport"));
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pos);
    pos
}

fn production(g: &mut Game, city: u32, value: f64) {
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        city,
        Yields {
            production: value,
            ..Default::default()
        },
    );
}

fn project(name: &str) -> Item {
    Item::Project {
        project: Name::new(name),
    }
}

#[test]
fn all_pads_start_lasers_in_one_pass_and_keep_invested_terrestrial_work() {
    let (mut g, a, b) = board();
    let terrestrial = project(LASERS[1]);
    g.apply(
        0,
        &Action::Produce {
            city: b,
            item: terrestrial.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&b).unwrap().production = 100.0;
    AdvancedAi::targeting(VictoryTarget::Science).science_production(&mut g, 0);
    assert_eq!(g.cities[&a].queue.first(), Some(&project(LASERS[0])));
    assert_eq!(g.cities[&b].queue.first(), Some(&terrestrial));
    assert_eq!(g.cities[&b].production, 100.0);
}

#[test]
fn faster_launch_city_reclaims_a_slow_assignment_without_duplicate_work() {
    let (mut g, a, b) = board();
    g.players[0].science_projects.remove("exoplanet_expedition");
    production(&mut g, a, 5.0);
    production(&mut g, b, 150.0);
    let launch = project("exoplanet_expedition");
    g.apply(
        0,
        &Action::Produce {
            city: a,
            item: launch.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&a).unwrap().production = 10.0;
    AdvancedAi::targeting(VictoryTarget::Science).science_production(&mut g, 0);
    assert_eq!(g.cities[&b].queue.first(), Some(&launch));
    assert_ne!(g.cities[&a].queue.first(), Some(&launch));
    assert_eq!(
        g.cities[&a].production_progress["project:exoplanet_expedition"],
        10.0
    );
}

#[test]
fn nearly_finished_launch_is_not_moved_to_a_higher_production_city() {
    let (mut g, a, b) = board();
    g.players[0].science_projects.remove("exoplanet_expedition");
    production(&mut g, a, 5.0);
    production(&mut g, b, 150.0);
    let launch = project("exoplanet_expedition");
    g.apply(
        0,
        &Action::Produce {
            city: a,
            item: launch.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&a).unwrap().production = g.item_cost_for_city(0, a, &launch) - 1.0;
    AdvancedAi::targeting(VictoryTarget::Science).science_production(&mut g, 0);
    assert_eq!(g.cities[&a].queue.first(), Some(&launch));
    assert_ne!(g.cities[&b].queue.first(), Some(&launch));
}

#[test]
fn pillaged_pad_does_not_hold_a_legal_launch_hostage() {
    let (mut g, a, b) = board();
    g.players[0].science_projects.remove("exoplanet_expedition");
    let launch = project("exoplanet_expedition");
    g.apply(
        0,
        &Action::Produce {
            city: a,
            item: launch.clone(),
        },
    )
    .unwrap();
    let pos = *g.cities[&a]
        .districts
        .get(crate::name!("spaceport"))
        .unwrap();
    g.map.tiles.get_mut(&pos).unwrap().pillaged = true;
    AdvancedAi::targeting(VictoryTarget::Science).science_production(&mut g, 0);
    assert_eq!(g.cities[&b].queue.first(), Some(&launch));
    assert_ne!(g.cities[&a].queue.first(), Some(&launch));
}

#[test]
fn laser_research_runs_while_expedition_is_still_under_construction() {
    let (mut g, _, _) = board();
    g.players[0].science_projects.remove("exoplanet_expedition");
    g.players[0].techs.remove(&crate::name!("offworld_mission"));
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(
        ai.science_endgame_research_goal(&g, 0),
        Some("offworld_mission")
    );
    g.players[0].techs.remove(&crate::name!("smart_materials"));
    assert_eq!(
        ai.science_endgame_research_goal(&g, 0),
        Some("smart_materials")
    );
}

#[test]
fn launched_flight_keeps_accelerating_even_at_the_turn_limit() {
    let (mut g, a, b) = board();
    g.max_turns = 100;
    g.turn = 99;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.score_horizon = true;
    assert!(ai.space_race_can_finish(&g, 0));
    ai.science_production(&mut g, 0);
    for city in [a, b] {
        assert_eq!(g.cities[&city].queue.first(), Some(&project(LASERS[0])));
    }
}

#[test]
fn frozen_controller_keeps_its_original_dispatch() {
    let (mut g, a, b) = board();
    assert!(!AdvancedAi::legacy().schedule_science_endgame(&mut g, 0));
    assert!(g.cities[&a].queue.is_empty());
    assert!(g.cities[&b].queue.is_empty());
}

#[test]
fn host_menu_can_offer_only_terrestrial_lasers() {
    let (mut g, a, _) = board();
    Arc::make_mut(&mut g.host_buildable).insert(
        a,
        [(
            "project:terrestrial_laser_station".to_string(),
            crate::game::HostMenuEntry {
                cost: Some(600.0),
                turns: Some(3.0),
            },
        )]
        .into_iter()
        .collect(),
    );
    AdvancedAi::targeting(VictoryTarget::Science).science_production(&mut g, 0);
    assert_eq!(g.cities[&a].queue.first(), Some(&project(LASERS[1])));
}

#[test]
fn host_completion_time_overrides_native_yield_estimates() {
    let (mut g, a, b) = board();
    g.players[0].science_projects.remove("exoplanet_expedition");
    production(&mut g, a, 500.0);
    production(&mut g, b, 5.0);
    for (city, turns) in [(a, 10.0), (b, 2.0)] {
        Arc::make_mut(&mut g.host_buildable).insert(
            city,
            [(
                "project:exoplanet_expedition".to_string(),
                crate::game::HostMenuEntry {
                    cost: Some(2100.0),
                    turns: Some(turns),
                },
            )]
            .into_iter()
            .collect(),
        );
    }
    AdvancedAi::targeting(VictoryTarget::Science).science_production(&mut g, 0);
    assert_eq!(
        g.cities[&b].queue.first(),
        Some(&project("exoplanet_expedition"))
    );
    assert!(g.cities[&a].queue.is_empty());
}

#[test]
fn next_pad_starts_on_the_same_turn_as_the_first_launch() {
    let (mut g, a, b) = board();
    g.players[0].science_projects.clear();
    let pos = *g.cities[&b]
        .districts
        .get(crate::name!("spaceport"))
        .unwrap();
    g.cities
        .get_mut(&b)
        .unwrap()
        .districts
        .remove(crate::name!("spaceport"));
    g.map.tiles.get_mut(&pos).unwrap().district = None;
    AdvancedAi::targeting(VictoryTarget::Science).science_production(&mut g, 0);
    assert_eq!(
        g.cities[&a].queue.first(),
        Some(&project("launch_earth_satellite"))
    );
    assert!(
        matches!(g.cities[&b].queue.first(), Some(Item::District { district, .. }) if district == "spaceport")
    );
}

#[test]
fn actual_research_dispatch_skips_old_optional_branches_for_the_launch() {
    let (mut g, _, _) = board();
    g.players[0].science_projects.remove("exoplanet_expedition");
    g.players[0].techs.remove(&crate::name!("smart_materials"));
    g.players[0].techs.remove(&crate::name!("offworld_mission"));
    g.players[0].techs.remove(&crate::name!("irrigation"));
    g.players[0].research = None;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    AdvancedAi::targeting(VictoryTarget::Science).advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("smart_materials"));
}

#[test]
fn completed_launch_hands_every_pad_to_lasers_on_the_next_decision() {
    let (mut g, a, b) = board();
    g.players[0].science_projects.remove("exoplanet_expedition");
    let launch = project("exoplanet_expedition");
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.science_production(&mut g, 0);
    let city = [a, b]
        .into_iter()
        .find(|cid| g.cities[cid].queue.first() == Some(&launch))
        .unwrap();
    g.cities.get_mut(&city).unwrap().production = g.item_cost_for_city(0, city, &launch) - 1.0;
    g.apply(0, &Action::EndTurn).unwrap();
    while g.current != 0 && !g.is_finished() {
        g.apply(g.current, &Action::EndTurn).unwrap();
    }
    assert!(g.players[0]
        .science_projects
        .contains("exoplanet_expedition"));
    ai.science_production(&mut g, 0);
    for city in [a, b] {
        assert_eq!(g.cities[&city].queue.first(), Some(&project(LASERS[0])));
    }
}

#[test]
fn repeated_parallel_lasers_finish_the_flight_before_the_old_one_queue_dispatch() {
    let (mut g, a, b) = board();
    g.victory_conditions = crate::game::VictoryConditions::parse("science").unwrap();
    for cid in [a, b] {
        production(&mut g, cid, 180.0);
    }
    let finish = |mut game: Game, modern: bool| {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        // Retain the original science_production path as the local control.
        ai.victory_planning = modern;
        for _ in 1..=60 {
            if game.current == 0 {
                ai.science_production(&mut game, 0);
            }
            game.apply(game.current, &Action::EndTurn).unwrap();
            if game.is_finished() {
                assert_eq!(game.winner, Some(0));
                return game.turn;
            }
        }
        panic!("the prepared flight should arrive within the scenario budget");
    };
    let old = finish(g.clone(), false);
    let new = finish(g, true);
    eprintln!("Prepared two-pad flight: old arrival turn {old}, optimized {new}");
    assert!(
        new < old,
        "parallel dispatch must advance actual arrival, not just queue more projects"
    );
}

#[test]
fn nearly_complete_expedition_and_parallel_lasers_fit_the_real_deadline() {
    let (mut g, a, _) = board();
    g.players[0].science_projects.remove("exoplanet_expedition");
    g.max_turns = 120;
    g.turn = 100;
    let launch = project("exoplanet_expedition");
    g.apply(
        0,
        &Action::Produce {
            city: a,
            item: launch.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&a).unwrap().production = g.item_cost_for_city(0, a, &launch) - 1.0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.score_horizon = true;
    assert!(ai.science_endgame_launch_fits(&g, 0));
    assert!(ai.space_race_can_finish(&g, 0));
    g.max_turns = 102;
    assert!(
        !ai.science_endgame_launch_fits(&g, 0),
        "the flight itself still takes time"
    );
}

#[test]
fn repairing_an_almost_finished_launch_beats_restarting_it_elsewhere() {
    let (mut g, a, b) = board();
    g.players[0].science_projects.remove("exoplanet_expedition");
    let launch = project("exoplanet_expedition");
    g.apply(
        0,
        &Action::Produce {
            city: a,
            item: launch.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&a).unwrap().production = g.item_cost_for_city(0, a, &launch) - 1.0;
    let pos = *g.cities[&a]
        .districts
        .get(crate::name!("spaceport"))
        .unwrap();
    g.map.tiles.get_mut(&pos).unwrap().pillaged = true;
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.science_production(&mut g, 0);
    ai.repair_stalled_science_project_queues(&mut g, 0);
    assert!(matches!(
        g.cities[&a].queue.first(),
        Some(Item::Repair { .. })
    ));
    assert_ne!(g.cities[&b].queue.first(), Some(&launch));
    assert!(g.cities[&a].production_progress["project:exoplanet_expedition"] > 0.0);
}

#[test]
fn third_pad_prepares_while_mars_is_still_being_built() {
    let (mut g, a, _) = board();
    let pos = g
        .map
        .tiles
        .iter()
        .find_map(|(pos, tile)| {
            (tile.owner_city.is_none()
                && g.rules.is_passable(tile)
                && !g.rules.is_water(tile)
                && g.cities.values().all(|city| g.wdist(*pos, city.pos) >= 6))
            .then_some(*pos)
        })
        .unwrap();
    let third = g.found_city_for(0, pos, None);
    g.cities.get_mut(&third).unwrap().pop = 12;
    g.players[0].science_projects.remove("launch_mars_colony");
    g.players[0].science_projects.remove("exoplanet_expedition");
    let mars = project("launch_mars_colony");
    g.apply(
        0,
        &Action::Produce {
            city: a,
            item: mars.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&a).unwrap().production = g.item_cost_for_city(0, a, &mars) - 1.0;
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.science_production(&mut g, 0);
    assert_eq!(g.cities[&a].queue.first(), Some(&mars));
    assert!(
        matches!(g.cities[&third].queue.first(), Some(Item::District { district, .. }) if district == "spaceport")
    );
}

#[test]
fn older_space_projects_do_not_override_another_explicit_victory_target() {
    let (mut g, _, _) = board();
    g.players[0].science_projects.remove("exoplanet_expedition");
    g.players[0].techs.remove(&crate::name!("smart_materials"));
    for target in [VictoryTarget::Culture, VictoryTarget::Diplomacy] {
        let ai = AdvancedAi::targeting(target);
        assert_eq!(ai.science_endgame_research_goal(&g, 0), None);
        assert!(!ai.schedule_science_endgame(&mut g, 0));
    }
    g.players[0]
        .science_projects
        .insert("exoplanet_expedition".into());
    assert!(AdvancedAi::targeting(VictoryTarget::Culture).science_endgame_committed(&g, 0));
}
