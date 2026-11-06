//! Tests of `advanced/early_archers.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

#[test]
fn exact_ground_search_prefers_the_high_value_kill_over_a_static_tie() {
    let mut game = Game::new_full(2, 24, 16, 71_009, 120, 0, false);
    game.at_war.insert((0, 1));
    let rival_origin = game
        .player_unit_ids(1)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .map(|unit| game.units[&unit].pos)
        .unwrap();
    game.found_city_for(1, rival_origin, None);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let (base, mut targets) = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.city_at(**position).is_none()
                && game
                    .cities
                    .values()
                    .all(|city| game.wdist(**position, city.pos) > 5)
        })
        .find_map(|(base, _)| {
            let targets: Vec<Pos> = game
                .nbrs(*base)
                .into_iter()
                .filter(|position| {
                    game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    }) && game.city_at(*position).is_none()
                })
                .collect();
            (targets.len() >= 2).then_some((*base, targets))
        })
        .expect("test map has an isolated two-target engagement");
    targets.sort_unstable();
    let robot = game.spawn_test_unit("giant_death_robot", 0, base);
    let warrior = game.spawn_test_unit("warrior", 1, targets[0]);
    let armor = game.spawn_test_unit("modern_armor", 1, targets[1]);
    game.units.get_mut(&warrior).unwrap().hp = 1;
    game.units.get_mut(&armor).unwrap().hp = 1;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);

    assert_eq!(
        ai.base.exchange_score(&game, robot, targets[0], true),
        ai.base.exchange_score(&game, robot, targets[1], true),
        "the static evaluator intentionally sees two one-hit kills"
    );
    let warrior_value = ai.tactical_attack_value(
        &game,
        0,
        robot,
        &Action::Ranged {
            unit: robot,
            target: targets[0],
        },
        &plan,
    );
    let armor_value = ai.tactical_attack_value(
        &game,
        0,
        robot,
        &Action::Ranged {
            unit: robot,
            target: targets[1],
        },
        &plan,
    );
    assert!(armor_value > warrior_value + 100.0);
    assert!(ai.advanced_military_step(&mut game, 0, robot, &plan));
    assert!(!game.units.contains_key(&armor));
    assert!(game.units.contains_key(&warrior));
    assert!(matches!(
        game.log.last(),
        Some((0, Action::Attack { target, .. } | Action::Ranged { target, .. }))
            if *target == targets[1]
    ));
}

#[test]
fn conquest_razes_a_hopeless_isolated_city_instead_of_recapturing_it() {
    let mut game = Game::new_full(2, 30, 18, 107_002, 120, 0, false);
    for pid in 0..2 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    let home = game.player_city_ids(0)[0];
    let rival_capital = game.player_city_ids(1)[0];
    let rival_pos = game.cities[&rival_capital].pos;
    let outpost_pos = game
        .wdisk(rival_pos, 4)
        .into_iter()
        .find(|position| {
            game.wdist(*position, rival_pos) == 4
                && game.wdist(*position, game.cities[&home].pos) > 9
                && game.city_at(*position).is_none()
                && game.map.tiles[position].owner_city.is_none()
        })
        .expect("test map has an isolated rival outpost site");
    {
        let tile = game.map.tiles.get_mut(&outpost_pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    game.cities.get_mut(&rival_capital).unwrap().pop = 15;
    game.cities.get_mut(&home).unwrap().pop = 3;
    let outpost = game.found_city_for(1, outpost_pos, Some("Revolt Loop".to_string()));
    {
        let captured = game.cities.get_mut(&outpost).unwrap();
        captured.owner = 0;
        captured.pop = 3;
        captured.loyalty = 50.0;
        captured.captured_from = Some(1);
        captured.occupied_from = Some(1);
    }
    game.current = 0;
    assert!(game
        .legal_city_disposition_actions(0)
        .iter()
        .any(|action| matches!(action, Action::RazeCity { city } if *city == outpost)));

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(AdvancedAi::population_loyalty_delta(&game, 0, outpost) <= -8.0);

    // A nearby core is not enough to save a tiny conquest that is already
    // forecast to revolt in three or four turns. The live Alexandria loop
    // was one population, six tiles from China, and changed hands five
    // times in seventeen turns because the older rule treated distance as
    // an absolute exemption.
    let mut nearby = game.clone();
    let near_home_pos = nearby
        .wdisk(outpost_pos, 6)
        .into_iter()
        .find(|position| {
            nearby.wdist(*position, outpost_pos) == 6
                && nearby.city_at(*position).is_none()
                && nearby.map.tiles[position].owner_city.is_none()
                && nearby.rules.is_passable(&nearby.map.tiles[position])
                && !nearby.rules.is_water(&nearby.map.tiles[position])
        })
        .expect("test map has a core site six tiles from the captured outpost");
    nearby.cities.get_mut(&home).unwrap().pos = near_home_pos;
    assert_eq!(
        nearby
            .cities
            .values()
            .filter(|city| city.owner == 0 && city.id != outpost)
            .map(|city| nearby.wdist(city.pos, outpost_pos))
            .min(),
        Some(6)
    );
    assert!(AdvancedAi::population_loyalty_delta(&nearby, 0, outpost) <= -8.0);
    let mut nearby_ai = AdvancedAi::targeting(VictoryTarget::Domination);
    nearby_ai.resolve_city_dispositions(&mut nearby, 0, GrandStrategy::Conquest);
    assert!(
        !nearby.cities.contains_key(&outpost),
        "an imminent low-value revolt should be razed even near a core city"
    );

    ai.resolve_city_dispositions(&mut game, 0, GrandStrategy::Conquest);

    assert!(
        !game.cities.contains_key(&outpost),
        "a city forecast to revolt before support can arrive should be razed once"
    );
}

/// See `DOOMED_CAPTURE_TURNS`: a small captured city six turns from revolt
/// at moderate pressure is kept by the shipped rules (neither imminent nor
/// isolated) and razed under `raze-a-doomed-capture`.
#[test]
fn a_doomed_small_capture_is_razed_under_the_gene() {
    let build = |gene: bool| {
        let mut game = Game::new_full(2, 30, 18, 107_002, 120, 0, false);
        for pid in 0..2 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        let home = game.player_city_ids(0)[0];
        let rival_capital = game.player_city_ids(1)[0];
        let rival_pos = game.cities[&rival_capital].pos;
        let outpost_pos = game
            .wdisk(rival_pos, 4)
            .into_iter()
            .find(|position| {
                game.wdist(*position, rival_pos) == 4
                    && game.wdist(*position, game.cities[&home].pos) > 9
                    && game.city_at(*position).is_none()
                    && game.map.tiles[position].owner_city.is_none()
            })
            .expect("test map has a rival outpost site");
        {
            let tile = game.map.tiles.get_mut(&outpost_pos).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
        game.cities.get_mut(&rival_capital).unwrap().pop = 14;
        game.cities.get_mut(&home).unwrap().pop = 3;
        let outpost = game.found_city_for(1, outpost_pos, Some("Bhavapura".to_string()));
        {
            let captured = game.cities.get_mut(&outpost).unwrap();
            captured.owner = 0;
            captured.pop = 4;
            captured.captured_from = Some(1);
            captured.occupied_from = Some(1);
        }
        game.current = 0;
        let near_home_pos = game
            .wdisk(outpost_pos, 6)
            .into_iter()
            .find(|position| {
                game.wdist(*position, outpost_pos) == 6
                    && game.city_at(*position).is_none()
                    && game.map.tiles[position].owner_city.is_none()
                    && game.rules.is_passable(&game.map.tiles[position])
                    && !game.rules.is_water(&game.map.tiles[position])
            })
            .expect("test map has a core site six tiles from the capture");
        game.cities.get_mut(&home).unwrap().pos = near_home_pos;
        let delta = AdvancedAi::population_loyalty_delta(&game, 0, outpost);
        assert!(
            delta <= DOOMED_CAPTURE_PRESSURE && delta > HEAVY_LOYALTY_PRESSURE,
            "fixture: moderate pressure ({delta})"
        );
        // Six turns from revolt: past the shipped four-turn rule, inside
        // the doomed-capture window.
        game.cities.get_mut(&outpost).unwrap().loyalty = 6.0 * -delta;
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        if gene {
            ai.enable_raze_doomed_capture();
        }
        ai.resolve_city_dispositions(&mut game, 0, GrandStrategy::Conquest);
        game.cities
            .get(&outpost)
            .is_some_and(|city| city.owner == 0)
    };
    assert!(build(false), "the shipped rules keep it");
    assert!(!build(true), "razed rather than handed back");
}
