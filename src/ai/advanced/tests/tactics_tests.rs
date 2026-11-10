//! Tests of `advanced/tactics.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

#[test]
fn timed_war_finisher_captures_the_open_objective_same_turn() {
    let (mut game, _, objective) = timed_war_fixture(940_010);
    clear_military(&mut game, 0);
    for unit in game.player_unit_ids(1) {
        game.remove_unit(unit);
    }
    game.at_war.insert((0, 1));
    game.cities.get_mut(&objective).unwrap().hp = 0;
    let objective_pos = game.cities[&objective].pos;
    let adjacent = game
        .nbrs(objective_pos)
        .into_iter()
        .find(|position| g_is_open_land(&game, *position) && game.unit_ids_at(*position).is_empty())
        .unwrap();
    let body = game.spawn_test_unit("giant_death_robot", 0, adjacent);
    let mut ai = AdvancedAi::new();
    ai.timed_war = true;
    let mut plan = timed_plan(
        &game,
        objective,
        crate::name!("nuclear_fusion"),
        crate::name!("giant_death_robot"),
        None,
        None,
        WarPhase::Exploit,
    );
    plan.declared_turn = Some(game.turn);
    ai.war_plan = Some(plan);
    assert_eq!(ai.timed_war_finisher_step(&mut game, 0, body), Some(true));
    assert_eq!(game.cities[&objective].owner, 0);
}

#[test]
fn fleet_objective_treats_embarked_enemies_as_naval_contacts() {
    let mut g = Game::new_full(2, 24, 16, 93, 80, 0, false);
    let (anchor, contact) = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| g.rules.is_water(tile))
        .find_map(|(pos, _)| {
            g.nbrs(*pos)
                .into_iter()
                .find(|neighbor| {
                    g.map
                        .get(*neighbor)
                        .is_some_and(|tile| g.rules.is_water(tile))
                })
                .map(|neighbor| (*pos, neighbor))
        })
        .expect("map has adjacent water");
    let _observer = g.spawn_test_unit("galley", 0, anchor);
    let embarked = g.spawn_test_unit("settler", 1, contact);
    assert!(
        g.player_can_see(0, contact),
        "the fleet can only target an embarked contact it can actually see"
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    let objective =
        AdvancedAi::new().domain_objective(&g, 0, &plan, ForceDomain::Sea, anchor, &[1]);
    assert_eq!(objective, g.units[&embarked].pos);
}

#[test]
fn fleet_uses_an_adjacent_water_approach_for_coastal_city_capture() {
    let mut g = Game::new_full(2, 24, 16, 94, 80, 0, false);
    for pid in 0..2 {
        g.current = pid;
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
    }
    g.current = 0;
    let target_city = g.player_city_ids(1)[0];
    let target = g.cities[&target_city].pos;
    let approach = g.nbrs(target)[0];
    {
        let tile = g.map.tiles.get_mut(&approach).unwrap();
        tile.terrain = crate::name!("coast");
        tile.feature = None;
        tile.hills = false;
    }
    g.players[0].techs.insert(crate::name!("sailing"));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target_city),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    let objective =
        AdvancedAi::new().domain_objective(&g, 0, &plan, ForceDomain::Sea, approach, &[1]);
    assert_eq!(g.wdist(objective, target), 1);
    assert!(g
        .map
        .get(objective)
        .is_some_and(|tile| g.rules.is_water(tile)));
}

#[test]
fn force_readiness_excludes_aircraft_from_ground_armies() {
    let mut game = Game::new_full(2, 24, 16, 71_004, 120, 0, false);
    game.at_war.insert((0, 1));
    let staging = game
        .map
        .tiles
        .iter()
        .find(|(position, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.unit_ids_at(**position).is_empty()
        })
        .map(|(position, _)| *position)
        .unwrap();
    let warrior = game.spawn_test_unit("warrior", 0, staging);
    let bomber = game.spawn_test_unit("bomber", 0, staging);
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

    ai.rebuild_force_groups(&game, 0, &plan);

    let army = ai
        .force_groups
        .iter()
        .find(|group| group.units.contains(&warrior))
        .expect("the ground unit forms an army order");
    assert!(!army.units.contains(&bomber));
    assert!(ai.force_groups.iter().all(|group| {
        group
            .units
            .iter()
            .all(|unit| game.rules.units[game.units[unit].kind].domain.as_deref() != Some("air"))
    }));
}

#[test]
fn command_radius_is_a_group_diameter_not_a_transitive_chain() {
    let mut game = Game::new_full(2, 24, 16, 71_005, 120, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    game.at_war.insert((0, 1));
    let chain = [
        game.spawn_test_unit("warrior", 0, (2, 5)),
        game.spawn_test_unit("warrior", 0, (8, 5)),
        game.spawn_test_unit("warrior", 0, (14, 5)),
    ];
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

    ai.rebuild_force_groups(&game, 0, &plan);

    assert!(ai.force_groups.iter().all(|group| {
        group.units.iter().all(|left| {
            group
                .units
                .iter()
                .all(|right| game.wdist(game.units[left].pos, game.units[right].pos) <= 6)
        })
    }));
    assert!(!ai
        .force_groups
        .iter()
        .any(|group| chain.iter().all(|unit| group.units.contains(unit))));
}

#[test]
fn local_superiority_prices_the_objective_city_defense() {
    let mut game = Game::new_full(2, 24, 16, 71_006, 120, 0, false);
    game.current = 1;
    let settler = game
        .player_unit_ids(1)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(1, &Action::FoundCity { unit: settler }).unwrap();
    let target_city = game.player_city_ids(1)[0];
    for unit in game.player_unit_ids(1) {
        game.remove_unit(unit);
    }
    game.players[1]
        .counters
        .insert("strongest_unit_built".to_string(), 80);
    let city_pos = {
        let city = game.cities.get_mut(&target_city).unwrap();
        city.buildings.push(crate::name!("walls"));
        city.wall_hp = 100;
        city.pos
    };
    let staging = game
        .nbrs(city_pos)
        .into_iter()
        .find(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
                && game.unit_ids_at(*position).is_empty()
        })
        .unwrap();
    let warrior = game.spawn_test_unit("warrior", 0, staging);
    game.at_war.insert((0, 1));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);

    let ratio = ai.local_strength_ratio(&game, 0, &[warrior], &[1], game.cities[&target_city].pos);

    assert!(
        ratio < 0.72,
        "one Warrior must not claim superiority over an intact defended city: {ratio}"
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target_city),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    ai.rebuild_force_groups(&game, 0, &plan);
    let order = ai
        .force_groups
        .iter()
        .find(|group| group.units.contains(&warrior))
        .unwrap();
    assert_eq!(order.posture, ForcePosture::Hold);
    assert_eq!(
        match order.posture {
            ForcePosture::Muster | ForcePosture::Hold | ForcePosture::Recover => order.anchor,
            ForcePosture::Engage => order.focus_target.unwrap_or(order.objective),
            ForcePosture::Advance => order.objective,
        },
        order.anchor,
        "an inferior force must hold its formation rather than target the city"
    );

    game.cities.get_mut(&target_city).unwrap().wall_hp = 0;
    game.cities.get_mut(&target_city).unwrap().hp = 1;
    ai.rebuild_force_groups(&game, 0, &plan);
    assert_eq!(
        ai.force_groups
            .iter()
            .find(|group| group.units.contains(&warrior))
            .unwrap()
            .posture,
        ForcePosture::Engage,
        "a forcing city capture must override the otherwise inferior local ratio"
    );
}

/// The defect this exists for: the movement score charged for the threat a
/// tile accepts and credited nothing for the attack it opens, so a melee
/// unit never volunteered for the tile it has to occupy to swing.
#[test]
fn a_tile_is_worth_the_attack_it_opens() {
    let (mut game, _capital, home) = empire_with_a_capital(71_106);
    let ours = game.spawn_test_unit("warrior", 0, anchor_at(&game, home, 2));
    let target = anchor_at(&game, home, 4);
    let theirs = game.spawn_test_unit("warrior", 1, target);

    let group = ForceGroup {
        id: ours,
        domain: ForceDomain::Land,
        units: vec![ours],
        anchor: game.units[&ours].pos,
        objective: target,
        focus_target: Some(target),
        posture: ForcePosture::Engage,
        readiness: 1.0,
        local_strength_ratio: 1.0,
    };
    let enemies = vec![1usize];
    let adjacent = game
        .nbrs(target)
        .into_iter()
        .find(|position| game.map.get(*position).is_some())
        .expect("a tile beside the enemy");

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.strike_opening, "the shipped default must be unchanged");
    assert_eq!(
        ai.strike_opening_value(StrikeOpening {
            g: &game,
            pid: 0,
            uid: ours,
            tile: adjacent,
            group: &group,
            enemies: &enemies,
            visible: None,
        }),
        0.0,
        "the shipped agent credits nothing"
    );

    ai.enable_strike_opening();
    let in_contact = ai.strike_opening_value(StrikeOpening {
        g: &game,
        pid: 0,
        uid: ours,
        tile: adjacent,
        group: &group,
        enemies: &enemies,
        visible: None,
    });
    assert!(
        in_contact > 0.0,
        "a tile that puts the target in reach must be worth something: {in_contact}"
    );
    // Two even warriors: odds are 0.5, so the credit is half the scale.
    assert!(
        (in_contact - STRIKE_OPENING_SCALE * 30.0 * 0.5).abs() < 1.5,
        "even odds should pay about half the scale, got {in_contact}"
    );

    // A tile out of reach opens nothing.
    let far = anchor_at(&game, home, 1);
    assert_eq!(
        ai.strike_opening_value(StrikeOpening {
            g: &game,
            pid: 0,
            uid: ours,
            tile: far,
            group: &group,
            enemies: &enemies,
            visible: None,
        }),
        0.0,
        "a tile the target cannot be struck from is worth nothing"
    );

    // Only while prosecuting. A group that is holding is not paid to close.
    let holding = ForceGroup {
        posture: ForcePosture::Hold,
        ..group.clone()
    };
    assert_eq!(
        ai.strike_opening_value(StrikeOpening {
            g: &game,
            pid: 0,
            uid: ours,
            tile: adjacent,
            group: &holding,
            enemies: &enemies,
            visible: None,
        }),
        0.0,
        "a held group must not be pulled into contact"
    );

    // A wounded defender is worth more than a fresh one, because the odds
    // of the strike are better.
    game.units.get_mut(&theirs).unwrap().hp = 30;
    let wounded = ai.strike_opening_value(StrikeOpening {
        g: &game,
        pid: 0,
        uid: ours,
        tile: adjacent,
        group: &group,
        enemies: &enemies,
        visible: None,
    });
    assert!(
        wounded > in_contact,
        "pouncing on a wounded target must score higher: {wounded} vs {in_contact}"
    );
}

#[test]
fn global_recovery_holds_an_unthreatened_front_but_keeps_a_forcing_finish() {
    // The t106 live-Rome shape: a declared major war and an enemy city
    // known outside current sight, but no local siege yet. The ordinary
    // fog-safe mover can still see a 3.00 local ratio here; Recovery must
    // make that a temporary home hold rather than an offensive march.
    let (mut game, _capital, home) = empire_with_a_capital(71_116);
    let enemy_site = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            game.wdist(**position, home) >= 15
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
        })
        .map(|(position, _)| *position)
        .min()
        .expect("fixture needs a distant enemy city site");
    game.current = 1;
    let enemy_settler = game.spawn_test_unit("settler", 1, enemy_site);
    game.apply(
        1,
        &Action::FoundCity {
            unit: enemy_settler,
        },
    )
    .expect("found the distant enemy city");
    game.current = 0;
    let target_city = *game
        .player_city_ids(1)
        .first()
        .expect("the enemy must own its founded city");
    let objective = game.cities[&target_city].pos;
    for unit in game.player_unit_ids(1) {
        game.remove_unit(unit);
    }
    let launch = game
        .nbrs(home)
        .into_iter()
        .find(|position| {
            game.map.get(*position).is_some_and(|tile| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.city_at(*position).is_none()
                    && game.unit_ids_at(*position).is_empty()
            })
        })
        .expect("the capital has an empty land launch tile");
    let warrior = game.spawn_test_unit("warrior", 0, launch);
    assert!(
        !game.player_can_see(0, objective),
        "the objective must be outside the turn-start sight frame"
    );

    let conquest = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target_city),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let recovery = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        ..conquest.clone()
    };
    let mut ai = AdvancedAi::new();
    ai.enable_bounded_recovery();
    assert!(
        ai.bounded_recovery,
        "the explicit bounded-Recovery treatment carries the bounded policy"
    );

    ai.rebuild_force_groups(&game, 0, &conquest);
    assert_eq!(
        ai.force_groups()
            .iter()
            .find(|group| group.units.contains(&warrior))
            .expect("the warrior receives a normal force order")
            .posture,
        ForcePosture::Advance,
        "outside Recovery the fogged city remains the ordinary campaign objective"
    );

    ai.rebuild_force_groups(&game, 0, &recovery);
    let holding = ai
        .force_groups()
        .iter()
        .find(|group| group.units.contains(&warrior))
        .expect("the warrior receives a Recovery force order");
    assert_eq!(holding.objective, objective);
    assert_eq!(
            holding.posture,
            ForcePosture::Hold,
            "an outmatched Recovery plan without a threatened city must not advance on a fogged campaign objective"
        );

    let mut legacy = AdvancedAi::legacy();
    legacy.rebuild_force_groups(&game, 0, &recovery);
    assert_eq!(
        legacy
            .force_groups()
            .iter()
            .find(|group| group.units.contains(&warrior))
            .expect("the frozen controller receives the same force order")
            .posture,
        ForcePosture::Advance,
        "the legacy anchor leaves bounded Recovery off and keeps its historical campaign movement"
    );

    // A finishable adjacent unit still overrides the home hold. This is
    // the existing forcing-focus exception, not a second attack policy.
    let contact = game
        .nbrs(launch)
        .into_iter()
        .find(|position| {
            game.map.get(*position).is_some_and(|tile| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.city_at(*position).is_none()
                    && game.unit_ids_at(*position).is_empty()
            })
        })
        .expect("the warrior has an empty adjacent land contact");
    let enemy = game.spawn_test_unit("warrior", 1, contact);
    game.units.get_mut(&enemy).unwrap().hp = 1;
    ai.rebuild_force_groups(&game, 0, &recovery);
    assert_eq!(
        ai.force_groups()
            .iter()
            .find(|group| group.units.contains(&warrior))
            .expect("the warrior keeps its force order at contact")
            .posture,
        ForcePosture::Engage,
        "Recovery may still finish a visible enemy it can immediately remove"
    );
}

#[test]
fn bomber_exact_result_search_prefers_a_kill_over_static_strength() {
    let mut game = Game::new_full(2, 24, 16, 71_005, 120, 0, false);
    game.at_war.insert((0, 1));
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    for unit in game.player_unit_ids(1) {
        game.remove_unit(unit);
    }
    let base = game
        .map
        .tiles
        .iter()
        .find(|(position, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.city_at(**position).is_none()
        })
        .map(|(position, _)| *position)
        .unwrap();
    let mut targets: Vec<Pos> =
        game.wdisk(base, game.rules.units["bomber"].range)
            .into_iter()
            .filter(|position| {
                *position != base
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
                    && game.city_at(*position).is_none()
            })
            .take(2)
            .collect();
    assert_eq!(targets.len(), 2);
    targets.sort_unstable();
    let bomber = game.spawn_test_unit("bomber", 0, base);
    game.spawn_test_unit("modern_armor", 1, targets[0]);
    let warrior = game.spawn_test_unit("warrior", 1, targets[1]);
    game.units.get_mut(&warrior).unwrap().hp = 1;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);

    assert_eq!(
        ai.advanced_air_action(&game, 0, bomber, &plan),
        Some(Action::AirStrike {
            unit: bomber,
            target: targets[1],
        })
    );
}

#[test]
fn bomber_planners_choose_high_value_air_pillage_over_low_value_strikes() {
    let mut game = Game::new_full(2, 24, 16, 71_006, 120, 0, false);
    game.at_war.insert((0, 1));
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let enemy_center = game
        .map
        .tiles
        .iter()
        .find(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(position, _)| *position)
        .unwrap();
    let enemy_city = game.found_city_for(1, enemy_center, None);
    let target = game
        .nbrs(enemy_center)
        .into_iter()
        .find(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        })
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&target).unwrap();
        tile.owner_city = Some(enemy_city);
        tile.improvement = Some(crate::name!("airstrip"));
        tile.pillaged = false;
    }
    let base =
        game.wdisk(target, game.rules.units["bomber"].range)
            .into_iter()
            .find(|position| {
                *position != target
                    && *position != enemy_center
                    && game.wdist(*position, target) >= 3
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
    game.found_city_for(0, base, None);
    let bomber = game.spawn_test_unit("bomber", 0, base);
    let expected = Action::AirPillage {
        unit: bomber,
        target,
    };
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(enemy_city),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };

    assert_eq!(
        BasicAi::new().doctrine_action(&game, 0, bomber),
        Some(expected.clone())
    );
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination)
            .advanced_air_action(&game, 0, bomber, &plan),
        Some(expected)
    );

    let capture_post = game
        .nbrs(enemy_center)
        .into_iter()
        .find(|position| *position != target && game.map.get(*position).is_some())
        .unwrap();
    game.spawn_test_unit("modern_armor", 0, capture_post);
    game.cities.get_mut(&enemy_city).unwrap().hp = 80;
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination)
            .advanced_air_action(&game, 0, bomber, &plan),
        Some(Action::AirStrike {
            unit: bomber,
            target: enemy_center,
        }),
        "a capture-ready original capital must not heal while bombers pillage"
    );
    let replan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_city: None,
        ..plan
    };
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination)
            .advanced_air_action(&game, 0, bomber, &replan),
        Some(Action::AirStrike {
            unit: bomber,
            target: enemy_center,
        }),
        "a second-front replan must not discard a nearby capital capture"
    );
}

#[test]
fn jet_planners_priority_target_escorted_air_defenses() {
    let mut game = Game::new_full(2, 24, 16, 71_007, 120, 0, false);
    game.at_war.insert((0, 1));
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let base = game
        .map
        .tiles
        .iter()
        .find(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(position, _)| *position)
        .unwrap();
    let target =
        game.wdisk(base, game.rules.units["jet_bomber"].range)
            .into_iter()
            .find(|position| {
                *position != base
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
    game.found_city_for(0, base, None);
    game.spawn_test_unit("modern_armor", 1, target);
    game.spawn_test_unit("mobile_sam", 1, target);
    let jet = game.spawn_test_unit("jet_bomber", 0, base);
    let expected = Action::PriorityTarget { unit: jet, target };
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };

    assert_eq!(
        BasicAi::new().doctrine_action(&game, 0, jet),
        Some(expected.clone())
    );
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination).advanced_air_action(&game, 0, jet, &plan),
        Some(expected)
    );
}

#[test]
fn armies_and_fleets_receive_domain_specific_shared_orders() {
    let mut g = Game::new_full(2, 24, 16, 78, 80, 0, false);
    g.at_war.insert((0, 1));

    let land_candidates = quietest_first(
        &g,
        g.map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && g.unit_ids_at(**pos).is_empty()
            })
            .map(|(pos, _)| *pos)
            .collect(),
    );
    let land_target = land_candidates
        .into_iter()
        .find_map(|pos| {
            let ring: Vec<Pos> = g
                .nbrs(pos)
                .into_iter()
                .filter(|neighbor| {
                    g.map.get(*neighbor).is_some_and(|tile| {
                        g.rules.is_passable(tile)
                            && !g.rules.is_water(tile)
                            && g.unit_ids_at(*neighbor).is_empty()
                    })
                })
                .collect();
            (ring.len() >= 3).then_some((pos, ring))
        })
        .expect("test map has an open land engagement");
    for position in [
        land_target.0,
        land_target.1[0],
        land_target.1[1],
        land_target.1[2],
    ] {
        let tile = g.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.improvement = None;
        tile.hills = false;
    }
    for ring_tile in land_target.1.iter().copied() {
        g.map.set_river_edge(land_target.0, ring_tile, false);
    }
    let army = [
        g.spawn_test_unit("warrior", 0, land_target.1[0]),
        g.spawn_test_unit("archer", 0, land_target.1[1]),
        g.spawn_test_unit("catapult", 0, land_target.1[2]),
    ];
    // A mirror matchup on level ground is an even trade, which the
    // planner is right to decline; this test is about which unit
    // receives the order, so leave the defender plainly worth striking.
    let defender = g.spawn_test_unit("warrior", 1, land_target.0);
    g.units.get_mut(&defender).unwrap().hp = 20;

    let sea_candidates = quietest_first(
        &g,
        g.map
            .tiles
            .iter()
            .filter(|(pos, tile)| g.rules.is_water(tile) && g.unit_ids_at(**pos).is_empty())
            .map(|(pos, _)| *pos)
            .collect(),
    );
    let sea_target = sea_candidates
        .into_iter()
        .find_map(|pos| {
            let ring: Vec<Pos> = g
                .nbrs(pos)
                .into_iter()
                .filter(|neighbor| {
                    g.map.get(*neighbor).is_some_and(|tile| {
                        g.rules.is_water(tile) && g.unit_ids_at(*neighbor).is_empty()
                    })
                })
                .collect();
            (ring.len() >= 2).then_some((pos, ring))
        })
        .expect("test map has an open naval engagement");
    let fleet = [
        g.spawn_test_unit("galley", 0, sea_target.1[0]),
        g.spawn_test_unit("galley", 0, sea_target.1[1]),
    ];
    g.spawn_test_unit("galley", 1, sea_target.0);

    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.rebuild_force_groups(&g, 0, &plan);

    let army_orders = ai
        .force_groups()
        .iter()
        .find(|group| army.iter().all(|uid| group.units.contains(uid)))
        .expect("combined-arms units should share one army order");
    assert_eq!(army_orders.domain, ForceDomain::Land);
    assert_eq!(army_orders.focus_target, Some(land_target.0));
    assert_eq!(army_orders.posture, ForcePosture::Engage);

    let fleet_orders = ai
        .force_groups()
        .iter()
        .find(|group| fleet.iter().all(|uid| group.units.contains(uid)))
        .expect("nearby ships should share one fleet order");
    assert_eq!(fleet_orders.domain, ForceDomain::Sea);
    assert_eq!(fleet_orders.objective, sea_target.0);
    assert_eq!(fleet_orders.focus_target, Some(sea_target.0));
    assert_eq!(fleet_orders.posture, ForcePosture::Engage);

    let acted = ai.advanced_military_step(&mut g, 0, army[0], &plan);
    let last = g.log.last().cloned();
    assert!(
        matches!(
            last,
            Some((0, Action::Attack { unit, target }))
                if unit == army[0] && target == land_target.0
        ),
        "the army's lead unit should strike the focus target: acted={acted}, log={last:?}"
    );
}

#[test]
fn city_state_wars_receive_a_campaign_target_and_combined_arms_orders() {
    let mut g = Game::new_full(2, 24, 16, 96, 80, 1, false);
    let minor = g
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .expect("test map has a city-state");
    let target_city = g.player_city_ids(minor)[0];
    let target = g.cities[&target_city].pos;
    let staging = g
        .nbrs(target)
        .into_iter()
        .find(|position| {
            g.map.get(*position).is_some_and(|tile| {
                g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && g.unit_ids_at(*position).is_empty()
            })
        })
        .expect("city-state needs an open attack front");
    // Three units, not two. The posture this case asserts turns on the
    // attackers being locally superior, and two of them cleared the 0.72
    // threshold by 0.009 against a city and its two defenders - close
    // enough that any change to the generated map decided the outcome.
    let attackers = [
        g.spawn_test_unit("warrior", 0, staging),
        g.spawn_test_unit("warrior", 0, staging),
        g.spawn_test_unit("archer", 0, staging),
    ];
    g.at_war.insert((0, minor));

    let mut ai = AdvancedAi::new();
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.target_player, Some(minor));
    assert_eq!(plan.target_city, Some(target_city));

    ai.rebuild_force_groups(&g, 0, &plan);
    let orders = ai
        .force_groups()
        .iter()
        .find(|group| attackers.iter().all(|unit| group.units.contains(unit)))
        .expect("the city-state front should form a shared army order");
    assert_eq!(orders.domain, ForceDomain::Land);
    assert_eq!(orders.objective, target);
    let focus = orders
        .focus_target
        .expect("the army should focus a city-state defender or its city");
    assert!(
        g.city_at(focus)
            .is_some_and(|city| g.cities[&city].owner == minor)
            || g.unit_ids_at(focus)
                .iter()
                .any(|unit| g.units[unit].owner == minor)
    );
    assert_eq!(orders.posture, ForcePosture::Engage);
}

#[test]
fn coordinated_force_moves_most_routed_units_on_advance() {
    let mut g = Game::new_full(2, 24, 16, 80, 80, 0, false);
    g.at_war.insert((0, 1));
    let (target, staging) = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.unit_ids_at(**pos).is_empty()
        })
        .find_map(|(target, _)| {
            let staging: Vec<Pos> = g
                .wdisk(*target, 6)
                .into_iter()
                .filter(|pos| {
                    (3..=6).contains(&g.wdist(*target, *pos))
                        && g.map.get(*pos).is_some_and(|tile| {
                            g.rules.is_passable(tile)
                                && !g.rules.is_water(tile)
                                && g.unit_ids_at(*pos).is_empty()
                        })
                })
                .take(6)
                .collect();
            (staging.len() == 6).then_some((*target, staging))
        })
        .expect("test map needs an open land campaign");
    g.spawn_test_unit("warrior", 1, target);
    let army: Vec<u32> = staging
        .into_iter()
        .map(|pos| g.spawn_test_unit("warrior", 0, pos))
        .collect();
    let orders = ForceGroup {
        id: army[0],
        domain: ForceDomain::Land,
        units: army.clone(),
        anchor: g.units[&army[0]].pos,
        objective: target,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    };
    let ai = AdvancedAi::new();
    for uid in &army {
        ai.coordinated_tactical_step(&mut g, 0, *uid, &orders, &[1], false);
    }
    let moved = army.iter().filter(|uid| g.units[uid].moved).count();
    assert!(
        moved * 2 > army.len(),
        "expected most coordinated troops to advance; moved {moved}/{}",
        army.len()
    );
}

#[test]
fn live_livelock_route_escape_can_cross_its_recorded_footprint() {
    let mut game = Game::new_full(2, 20, 14, 80_271, 80, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    // Match the router's obstacle fixture: all geometrically improving
    // first steps are mountains, so the valid route begins sideways or
    // backward. The tactical score is deliberately allowed to dislike
    // that step; six observed fruitless turns are the evidence that the
    // short detour is now better than another greedy retry.
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    let start = *game
        .map
        .tiles
        .keys()
        .find(|position| game.wdisk(**position, 3).len() == 37)
        .expect("fixture needs an interior tile");
    let target = (start.0 + 3, start.1);
    let unit = game.spawn_test_unit("warrior", 0, start);
    game.at_war.insert((0, 1));
    let direct = game
        .nbrs(start)
        .into_iter()
        .filter(|position| game.wdist(*position, target) < game.wdist(start, target))
        .collect::<Vec<_>>();
    for position in direct {
        game.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("mountain");
    }
    let detour = game
        .route_step(unit, target, 1)
        .expect("the mountain wedge leaves a route to attack range");
    assert!(
        game.wdist(detour, target) >= game.wdist(start, target),
        "the first route step is a genuine geometric detour"
    );

    let mut live = AdvancedAi::new();
    live.enable_recorded_tactical_step();
    // Record the same two-tile pocket at the start of six real turns.
    // The route's first step is deliberately inside that footprint, which
    // is exactly the case `path_move` must ordinarily reject.
    for position in [start, detour, start, detour, start, detour, start] {
        let mover = game.units.get_mut(&unit).unwrap();
        mover.pos = position;
        mover.moves_left = 2.0;
        mover.moved = false;
        live.base.begin_movement_turn(&game, 0);
        game.turn += 1;
    }
    assert!(live.base.live_livelock_route_escape(unit));
    assert!(
        !live.base.tactical_apply_move(&mut game, 0, unit, detour),
        "ordinary recorded movement must keep rejecting the exhausted footprint"
    );

    let orders = ForceGroup {
        id: unit,
        domain: ForceDomain::Land,
        units: vec![unit],
        anchor: start,
        objective: target,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    };
    assert!(live.coordinated_tactical_step(&mut game, 0, unit, &orders, &[1], false));
    assert_eq!(
        game.units[&unit].pos, detour,
        "the live route exception may retrace one recorded tile only to leave the pocket"
    );
    assert!(matches!(
        game.log.last(),
        Some((0, Action::Move { unit: moved, to })) if *moved == unit && *to == detour
    ));
}

#[test]
fn coordinated_mover_ignores_a_hostile_missing_from_its_turn_start_frame() {
    let choose = |ai: &AdvancedAi, game: &mut Game, unit: u32, orders: &ForceGroup| {
        ai.coordinated_tactical_step(game, 0, unit, orders, &[1], false)
            .then_some(())?;
        match game.log.last() {
            Some((actor, Action::Move { unit: moved, to })) if *actor == 0 && *moved == unit => {
                Some(*to)
            }
            _ => None,
        }
    };

    // Find a real open tactical fork, then prove that a threat beside the
    // ordinary move is material to the legacy mover.  The production
    // mover captures its frame before that hostile exists, which is the
    // same information boundary as a hidden enemy changing position or
    // HP in the counterfactual census.
    let mut game = Game::new_full(2, 24, 16, 82_000, 80, 0, false);
    let existing: Vec<u32> = game.units.keys().copied().collect();
    for unit in existing {
        game.remove_unit(unit);
    }
    // A level plain makes this a fixed local fork rather than a search
    // over map generation. Only the tactical score varies below.
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    game.at_war.insert((0, 1));
    let positions = game.map.tiles.keys().copied().collect::<Vec<_>>();
    let origin = positions
        .iter()
        .copied()
        .find(|origin| game.nbrs(*origin).len() == 6)
        .expect("the fixed map needs an interior tactical origin");
    let (game, unit, orders, expected, hidden) = game
        .wdisk(origin, 4)
        .into_iter()
        .filter(|target| game.wdist(origin, *target) == 4)
        .find_map(|target| {
            let mut base = game.clone();
            let unit = base.spawn_test_unit("warrior", 0, origin);
            let orders = ForceGroup {
                id: unit,
                domain: ForceDomain::Land,
                units: vec![unit],
                anchor: origin,
                objective: target,
                focus_target: None,
                posture: ForcePosture::Advance,
                readiness: 1.0,
                local_strength_ratio: 2.0,
            };
            let mut unthreatened = base.clone();
            let expected = choose(&AdvancedAi::legacy(), &mut unthreatened, unit, &orders)?;
            game.wdisk(expected, 3)
                .into_iter()
                .filter(|position| game.wdist(*position, expected) == 3)
                .filter(|hidden| *hidden != origin && *hidden != target)
                .find_map(|hidden| {
                    let mut legacy = base.clone();
                    legacy.spawn_test_unit("giant_death_robot", 1, hidden);
                    let detour = choose(&AdvancedAi::legacy(), &mut legacy, unit, &orders)?;
                    (detour != expected).then_some((
                        base.clone(),
                        unit,
                        orders.clone(),
                        expected,
                        hidden,
                    ))
                })
        })
        .expect("the fixed open map needs a tactical fork affected by a hostile threat");

    let mut observed = game;
    let mut ai = AdvancedAi::new();
    ai.capture_battlefront_frame(&observed, 0);
    let enemy = observed.spawn_test_unit("giant_death_robot", 1, hidden);
    assert!(
        !ai.battlefront_unit_visible(&observed, 0, enemy),
        "the threat was added after the turn-start frame"
    );
    assert_eq!(
        choose(&ai, &mut observed, unit, &orders),
        Some(expected),
        "a hostile absent from the turn-start frame must not reroute the force"
    );
}

#[test]
fn recon_explores_independently_while_combat_roles_form_the_army() {
    let mut g = Game::new_full(2, 24, 16, 81, 80, 0, false);
    g.at_war.insert((0, 1));
    let positions: Vec<Pos> = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.unit_ids_at(**pos).is_empty()
        })
        .map(|(pos, _)| *pos)
        .take(6)
        .collect();
    let scout = g.spawn_test_unit("scout", 0, positions[0]);
    let vanguard = g.spawn_test_unit("swordsman", 0, positions[1]);
    let mobile = g.spawn_test_unit("horseman", 0, positions[2]);
    let ranged = g.spawn_test_unit("archer", 0, positions[3]);
    let siege = g.spawn_test_unit("catapult", 0, positions[4]);
    let support = g.spawn_test_unit("battering_ram", 0, positions[5]);
    assert_eq!(AdvancedAi::force_role(&g, scout), ForceRole::Recon);
    assert_eq!(AdvancedAi::force_role(&g, vanguard), ForceRole::Vanguard);
    assert_eq!(AdvancedAi::force_role(&g, mobile), ForceRole::Mobile);
    assert_eq!(AdvancedAi::force_role(&g, ranged), ForceRole::Ranged);
    assert_eq!(AdvancedAi::force_role(&g, siege), ForceRole::Siege);
    assert_eq!(AdvancedAi::force_role(&g, support), ForceRole::Support);

    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.rebuild_force_groups(&g, 0, &plan);
    assert!(
        ai.force_groups()
            .iter()
            .all(|group| !group.units.contains(&scout)),
        "recon with unexplored terrain should not make the army wait to muster"
    );
    assert!(ai
        .force_groups()
        .iter()
        .any(|group| group.units.contains(&vanguard)));
}

#[test]
fn force_replans_focus_after_each_battlefield_action() {
    let mut g = Game::new_full(2, 24, 16, 83, 80, 0, false);
    // Only the staged attackers belong in this force. Organic starting
    // units can otherwise split them into separate command-radius groups.
    for unit in g.player_unit_ids(0) {
        g.remove_unit(unit);
    }
    g.at_war.insert((0, 1));
    let front_candidates = quietest_first(
        &g,
        g.map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && g.unit_ids_at(**pos).is_empty()
            })
            .map(|(pos, _)| *pos)
            .collect(),
    );
    let (first_target, second_target, firing_line) = front_candidates
        .into_iter()
        .find_map(|first| {
            g.nbrs(first).into_iter().find_map(|second| {
                let second_tile = g.map.get(second)?;
                if !g.rules.is_passable(second_tile)
                    || g.rules.is_water(second_tile)
                    || !g.unit_ids_at(second).is_empty()
                {
                    return None;
                }
                let second_neighbors = g.nbrs(second);
                let common: Vec<Pos> = g
                    .nbrs(first)
                    .into_iter()
                    .filter(|pos| second_neighbors.contains(pos))
                    .filter(|pos| {
                        g.map.get(*pos).is_some_and(|tile| {
                            g.rules.is_passable(tile)
                                && !g.rules.is_water(tile)
                                && g.unit_ids_at(*pos).is_empty()
                        })
                    })
                    .collect();
                (common.len() >= 2).then_some((first, second, common))
            })
        })
        .expect("test map has a two-target engagement with a shared front");

    // Level the arena: the test exercises replanning after a kill, and
    // must not hinge on whichever defense modifiers the organic map put
    // under the four staged tiles.
    for position in [first_target, second_target, firing_line[0], firing_line[1]] {
        let tile = g.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }

    let attackers = [
        g.spawn_test_unit("warrior", 0, firing_line[0]),
        g.spawn_test_unit("warrior", 0, firing_line[1]),
    ];
    let first_enemy = g.spawn_test_unit("warrior", 1, first_target);
    g.units.get_mut(&first_enemy).unwrap().hp = 1;
    let second_enemy = g.spawn_test_unit("warrior", 1, second_target);
    g.units.get_mut(&second_enemy).unwrap().hp = 1;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.rebuild_force_groups(&g, 0, &plan);
    let initial = ai
        .force_groups()
        .iter()
        .find(|group| attackers.iter().all(|uid| group.units.contains(uid)))
        .unwrap();
    assert_eq!(initial.focus_target, Some(first_target));

    assert!(ai.advanced_military_step(&mut g, 0, attackers[0], &plan));
    assert!(!g.units.contains_key(&first_enemy));
    assert!(ai.advanced_military_step(&mut g, 0, attackers[1], &plan));
    let replanned = ai
        .force_groups()
        .iter()
        .find(|group| group.units.contains(&attackers[1]))
        .unwrap();
    assert_eq!(replanned.focus_target, Some(second_target));
    assert!(
        matches!(
            g.log.last(),
            Some((0, Action::Attack { unit, target }))
                if *unit == attackers[1] && *target == second_target
        ),
        "unexpected replanned action log: {:?}",
        g.log
    );
}

#[test]
fn a_charted_village_preempts_a_prewar_campaign_staging_order() {
    let mut game = Game::new_full(2, 30, 20, 90_081, 120, 0, false);
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }

    let mut land: Vec<Pos> = game.map.tiles.keys().copied().collect();
    land.sort();
    let (home, objective_site) = land
        .iter()
        .flat_map(|home| land.iter().map(move |objective| (*home, *objective)))
        .find(|(home, objective)| {
            game.wdist(*home, *objective) >= 16
                && game.wdisk(*home, 5).len() >= 24
                && game.wdisk(*objective, 5).len() >= 24
        })
        .expect("the map has two separated interior city sites");
    for (player, site) in [(0, home), (1, objective_site)] {
        game.current = player;
        let settler = game.spawn_test_unit("settler", player, site);
        game.apply(player, &Action::FoundCity { unit: settler })
            .expect("each site can found a city");
    }
    game.current = 0;
    game.record_contact(0, 1);
    let target_city = game.player_city_ids(1)[0];
    let objective = game.cities[&target_city].pos;
    let open = |g: &Game, pos: Pos| {
        g.map
            .get(pos)
            .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            && g.city_at(pos).is_none()
            && g.unit_ids_at(pos).is_empty()
    };
    let (assault_at, village) = game
        .wdisk(home, 4)
        .into_iter()
        .filter(|pos| open(&game, *pos) && game.wdist(*pos, home) == 4)
        .find_map(|assault_at| {
            game.wdisk(assault_at, crate::ai::VILLAGE_MILITARY_SEEK_RADIUS)
                .into_iter()
                .filter(|pos| {
                    open(&game, *pos)
                        && game.wdist(assault_at, *pos) == crate::ai::VILLAGE_MILITARY_SEEK_RADIUS
                        && game.wdist(*pos, objective) > game.wdist(assault_at, objective)
                })
                .max()
                .map(|village| (assault_at, village))
        })
        .expect("an open field unit can reach a village away from the campaign objective");
    game.map.tiles.get_mut(&village).unwrap().improvement = Some(crate::name!("goody_hut"));
    let assault = game.spawn_test_unit("warrior", 0, assault_at);
    game.players[0]
        .explored
        .extend(game.map.tiles.keys().copied());
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target_city),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.battlefront_observation = false;
    assert!(ai.base.village_seeking);

    let before = game.wdist(assault_at, village);
    let mut historic = game.clone();
    let mut historic_ai = ai.clone();
    assert_eq!(
        historic_ai.campaign_staging_step(&mut historic, 0, assault, &plan),
        Some(true),
        "precondition: without the village priority this Warrior would stage for war"
    );
    assert!(
        historic.wdist(historic.units[&assault].pos, village) >= before,
        "the staging route must not accidentally make progress toward the village"
    );

    assert!(ai.advanced_military_step(&mut game, 0, assault, &plan));
    assert!(
        game.wdist(game.units[&assault].pos, village) < before,
        "the nearby Warrior should collect the charted village before staging for war"
    );
}

/// ★★★★★ A RELIEF COLUMN MUST AIM AT THE SIEGE, NOT AT ITS OWN DOORSTEP.
///
/// `domain_objective`'s `threatened_city` arm gathers every hostile within 8
/// of the besieged city and then takes the one nearest the RELIEVING FORCE.
/// A force standing east of a burning city therefore aims at the easternmost
/// besieger — on the far side of the ring — and two force groups anchored in
/// different places aim at different enemies, so the army never converges.
///
/// ⚠⚠⚠ Measured live on run `civvis-20260803T220954Z`, t180–187, with the
/// embarkation, visibility and border repairs all running. Mediolanum went
/// 0 → 29 → 98 → 167 → 180 damage out of 200 in four turns with eight
/// hostiles inside five hexes; **five of our nine combat units stood 8 to 11
/// hexes away**, and the journalled objectives were all on the far side of
/// the ring. The city fell and the run ended in defeat holding one city.
#[test]
fn a_relief_column_aims_at_the_siege_not_at_its_own_doorstep() {
    let mut game = Game::new_full(2, 30, 18, 411_011, 120, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    game.apply(0, &Action::DeclareWar { player: 1 })
        .expect("the seats can go to war");

    // Our city, and a straight run of land east of it so the geometry is
    // unambiguous: the besieger is adjacent to the city, the loiterer is far
    // from the city but close to where our force is standing.
    let mut land: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !game.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    land.sort();
    let city_pos = land[0];
    let settler = game.spawn_test_unit("settler", 0, city_pos);
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("the city can be founded");
    let cid = game.player_city_ids(0)[0];

    let pick = |d: i32, from: Pos| -> Pos {
        *land
            .iter()
            .find(|pos| game.wdist(**pos, from) == d && **pos != city_pos)
            .unwrap_or_else(|| panic!("a land tile {d} from {from:?}"))
    };
    let besieger_pos = pick(1, city_pos);
    let anchor = pick(7, city_pos);
    let loiterer_pos = pick(1, anchor);

    game.spawn_test_unit("warrior", 1, besieger_pos);
    game.spawn_test_unit("warrior", 1, loiterer_pos);
    let ours = game.spawn_test_unit("warrior", 0, anchor);

    // Both hostiles are inside the city's 8-ring, so both are candidates;
    // the only question is which one the column is sent to.
    assert!(
        game.wdist(city_pos, loiterer_pos) <= 8,
        "both are candidates"
    );

    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: Some(cid),
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let enemies = [1usize];

    let mut ai = AdvancedAi::new();
    // The candidate filter honours fog when battlefront observation is on,
    // and this test is about the ORDERING, not about what can be seen.
    ai.battlefront_observation = false;
    let _ = ours;

    ai.disable_relief_targets_the_siege();
    let nearest_to_us = ai.domain_objective(&game, 0, &plan, ForceDomain::Land, anchor, &enemies);
    ai.enable_relief_targets_the_siege();
    let nearest_to_city = ai.domain_objective(&game, 0, &plan, ForceDomain::Land, anchor, &enemies);

    assert_eq!(
        nearest_to_us, loiterer_pos,
        "the historical rule aims at whatever stands nearest the force — this is \
             the defect, and it left five of nine units 8 to 11 hexes from a city at \
             180 damage out of 200"
    );
    assert_eq!(
        nearest_to_city, besieger_pos,
        "the relief column must aim at what is actually hitting the city"
    );
}

/// ★★★★★ A RELIEF COLUMN MARCHES; THE RING HOLDS.
///
/// Run `civvis-20260827T113726Z`, t161–164: Aquileia (pop 15, walled, no
/// garrison) under a Chinese Artillery, Field Cannon, three Infantry and two
/// Cuirassiers. Our main army — seven units, **1.58** local strength against
/// that stack, objective the besieger's own hex at (20,30) — logged "A land
/// force of 7 will hold … held back to cover a threat" on every frame of
/// every turn and stood at x=7–13, nine hexes west, until the city fell on
/// t164. `relieving` was `plan.threatened_city.is_some()` — true for every
/// group in the empire — and a Hold's mover target is the group's own
/// anchor. #354 measured the reach test, shipped it off, and #1194 pruned
/// it.
///
/// With the gene: a group beyond `THREAT_RELIEF_RADIUS` of the threatened
/// city that is locally superior advances on the siege; a group inside the
/// radius holds its ring exactly as before; the frozen anchor is untouched.
#[test]
fn a_relief_column_beyond_the_threat_radius_advances_on_the_siege() {
    let (mut game, capital, home) = empire_with_a_capital(71_131);
    let besieger_pos = clear_land_at(&game, home, 1, &[], 0);
    game.spawn_test_unit("warrior", 1, besieger_pos);
    // The column: two warriors nine hexes out, side by side, so the group
    // is fully mustered and twice the besieger's strength.
    let far = clear_land_at(&game, home, 9, &[], 1);
    let far_two = clear_land_beside(&game, far, &[]);
    let column_a = game.spawn_test_unit("warrior", 0, far);
    let column_b = game.spawn_test_unit("warrior", 0, far_two);
    // The ring: one warrior three hexes out, out of contact with the
    // besieger and out of the column's command radius.
    let near = clear_land_at(
        &game,
        home,
        3,
        &[(besieger_pos, 3), (far, 7), (far_two, 7)],
        0,
    );
    let ring = game.spawn_test_unit("warrior", 0, near);

    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: Some(capital),
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let group_of = |ai: &AdvancedAi, unit: u32| -> ForceGroup {
        ai.force_groups()
            .iter()
            .find(|group| group.units.contains(&unit))
            .cloned()
            .unwrap_or_else(|| panic!("unit {unit} receives a force order"))
    };

    let mut ai = AdvancedAi::new();
    // The geometry is the question, not what the turn-start frame saw.
    ai.battlefront_observation = false;
    assert!(
        !ai.relief_column_marches,
        "a repair gene ships off in the constructor"
    );

    ai.rebuild_force_groups(&game, 0, &plan);
    let column = group_of(&ai, column_a);
    assert!(column.units.contains(&column_b), "the pair is one group");
    assert!(
        !column.units.contains(&ring),
        "the ring warrior is not in the column"
    );
    assert!(
        game.wdist(column.anchor, home) > THREAT_RELIEF_RADIUS,
        "the column stands beyond the relief radius: {}",
        game.wdist(column.anchor, home)
    );
    assert!(
        column.local_strength_ratio >= LOCAL_SUPERIORITY_FLOOR,
        "two warriors against one are locally superior: {}",
        column.local_strength_ratio
    );
    assert_eq!(
        column.objective, besieger_pos,
        "the objective is the besieger"
    );
    assert_eq!(
        column.posture,
        ForcePosture::Hold,
        "the shipped rule: every group holds while any city is threatened — this \
         is the defect, nine hexes from a city at 145 damage out of 200"
    );
    assert_eq!(group_of(&ai, ring).posture, ForcePosture::Hold);
    assert_eq!(
        ai.relief_hold_point(&game, &column),
        None,
        "with the gene off a Hold stands at its anchor"
    );

    ai.enable_relief_column_marches();
    ai.rebuild_force_groups(&game, 0, &plan);
    let column = group_of(&ai, column_a);
    assert_eq!(
        column.posture,
        ForcePosture::Advance,
        "a locally superior column beyond the relief radius marches on the siege"
    );
    assert_eq!(column.objective, besieger_pos);
    let ring_group = group_of(&ai, ring);
    assert!(game.wdist(ring_group.anchor, home) <= THREAT_RELIEF_RADIUS);
    assert_eq!(
        ring_group.posture,
        ForcePosture::Hold,
        "a group inside the radius is part of the defence and holds its ring"
    );
    // Where a Hold stands: the column at the threatened city, the ring at
    // its own anchor.
    ai.plan = Some(plan.clone());
    assert_eq!(
        ai.relief_hold_point(&game, &column),
        Some(home),
        "a column too weak to advance would still hold AT the city"
    );
    assert_eq!(ai.relief_hold_point(&game, &ring_group), None);

    let mut legacy = AdvancedAi::legacy();
    legacy.battlefront_observation = false;
    legacy.rebuild_force_groups(&game, 0, &plan);
    assert_eq!(
        group_of(&legacy, column_a).posture,
        ForcePosture::Hold,
        "the frozen anchor keeps the empire-wide hold"
    );
}

/// The t160 frame of the same run: the army's posture read "Engage" with
/// its objective at the Chinese Cuirassier, but the focus — the tile the
/// Engage mover walks to — was a Chinese Apostle standing beside the
/// crossbowmen, which every unit then "declines" (worth −∞). While a city is
/// threatened, a column's focus is a combatant or a city; at peace the gene
/// changes nothing.
#[test]
fn a_relief_columns_focus_is_a_combatant_not_a_civilian() {
    let (mut game, capital, home) = empire_with_a_capital(71_132);
    let stand = clear_land_at(&game, home, 5, &[], 2);
    let ours = game.spawn_test_unit("warrior", 0, stand);
    let beside = clear_land_beside(&game, stand, &[]);
    game.spawn_test_unit("builder", 1, beside);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: Some(capital),
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let calm = StrategicPlan {
        threatened_city: None,
        ..plan.clone()
    };
    let enemies = [1usize];
    let mut ai = AdvancedAi::new();
    ai.battlefront_observation = false;

    assert_eq!(
        ai.force_focus_target(&game, 0, &[ours], &enemies, &plan),
        Some(beside),
        "the shipped rule focuses on any enemy tile in reach"
    );
    ai.enable_relief_column_marches();
    assert_eq!(
        ai.force_focus_target(&game, 0, &[ours], &enemies, &plan),
        None,
        "a Builder is not what is shelling the city"
    );
    assert_eq!(
        ai.force_focus_target(&game, 0, &[ours], &enemies, &calm),
        Some(beside),
        "with no city threatened the gene is inert"
    );
    let other = clear_land_beside(&game, stand, &[beside]);
    game.spawn_test_unit("warrior", 1, other);
    assert_eq!(
        ai.force_focus_target(&game, 0, &[ours], &enemies, &plan),
        Some(other),
        "an enemy warrior in reach is still the focus"
    );
}
