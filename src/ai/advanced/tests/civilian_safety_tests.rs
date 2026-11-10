//! Tests of `advanced/civilian_safety.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

/// A targeted Science seat must finish the cheap half of a Campus before the
/// strategic governor spends the city's queue on an unrelated district or
/// building. The explicit target is the contract here; the independently
/// screenable reserve gene remains off.
#[test]
fn an_explicit_science_target_reserves_its_owed_campus_building() {
    let (mut game, city, _) = empire_with_a_capital(71_124);
    clear_barbarian_fixture(&mut game);
    install_ai_test_district(&mut game, city, "campus");
    install_ai_test_district(&mut game, city, "theater_square");
    game.cities.get_mut(&city).unwrap().buildings =
        vec![crate::name!("monument"), crate::name!("walls")];
    game.players[0].techs.insert(crate::name!("writing"));
    game.turn = game.max_turns / 2;
    let library = Item::Building {
        building: crate::name!("library"),
    };
    assert!(game.can_produce(0, city, &library), "the Library is legal");
    assert!(game.cities[&city].queue.is_empty(), "the queue starts idle");

    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    assert!(
        ai.research_economy,
        "the targeted controller prices research"
    );
    assert!(
        !ai.first_research_building_reserve,
        "the independently screenable reserve stays off"
    );
    ai.advanced_production(&mut game, 0, &plan, false);
    assert_eq!(
        game.cities[&city].queue.first(),
        Some(&library),
        "Science must reserve the owed Campus building before generic production"
    );
}

#[test]
fn conquest_ai_spends_a_device_on_the_hard_city_but_spares_its_own() {
    let mut game = Game::new_full(2, 24, 16, 91_802, 200, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let target_city = game.player_city_ids(1)[0];
    let target = game.cities[&target_city].pos;
    game.at_war.insert((0, 1));
    game.install_test_silo(game.player_city_ids(0)[0]);
    game.players[0]
        .counters
        .insert("project_effect:thermonuclear_devices".to_string(), 1);
    game.players[0].explored.insert(target);
    game.cities.get_mut(&target_city).unwrap().wall_hp = 300;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target_city),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);

    // A friendly scout in the blast must hold the launch.
    let radius = game.rules.wmds["thermonuclear_device"].blast_radius;
    let picket_pos =
        game.wdisk(target, radius)
            .into_iter()
            .find(|position| {
                *position != target
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
                    && game.unit_ids_at(*position).is_empty()
            })
            .expect("blast ring has an open land tile");
    let picket = game.spawn_test_unit("scout", 0, picket_pos);
    ai.advanced_wmd_strikes(&mut game, 0, &plan);
    assert_eq!(
        game.players[0].counters["project_effect:thermonuclear_devices"], 1,
        "no launch while a friendly unit stands in the blast"
    );

    game.remove_unit(picket);
    ai.advanced_wmd_strikes(&mut game, 0, &plan);
    assert_eq!(
        game.players[0].counters["project_effect:thermonuclear_devices"], 0,
        "the hard city draws the device once the blast is clean"
    );
    assert!(game.map.tiles[&target].fallout_until > game.turn);
}

#[test]
fn the_doctrine_spares_a_power_it_is_not_fighting() {
    let mut game = Game::new_full(3, 24, 16, 91_802, 200, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    // Exactly the units this test places, nothing else in any blast.
    for pid in 0..3 {
        for unit in game.player_unit_ids(pid) {
            game.remove_unit(unit);
        }
    }
    let launch_pos = game.cities[&game.player_city_ids(0)[0]].pos;
    let rich_city = game.player_city_ids(1)[0];
    let rich = game.cities[&rich_city].pos;
    let radius = game.rules.wmds["thermonuclear_device"].blast_radius;
    // A second, quieter city for the same belligerent, far enough from
    // every other stake that its blast disk is clean.
    let plain = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
                && game.wdist(*position, rich) > 2 * radius + 1
                && game.wdist(*position, launch_pos) > radius
                && game.wdist(*position, launch_pos) <= 12
                && game
                    .wdisk(*position, radius)
                    .into_iter()
                    .all(|tile| game.map.get(tile).is_some_and(|t| t.owner_city.is_none()))
        })
        .min_by_key(|position| (game.wdist(*position, launch_pos), position.0, position.1))
        .expect("the map has a clean tile for the second city");
    let plain_city = game.found_city_for(1, plain, None);
    game.at_war.insert((0, 1));
    game.install_test_silo(game.player_city_ids(0)[0]);
    game.players[0]
        .counters
        .insert("project_effect:thermonuclear_devices".to_string(), 1);
    game.players[0].explored.insert(rich);
    game.players[0].explored.insert(plain);
    game.cities.get_mut(&rich_city).unwrap().wall_hp = 400;
    game.cities.get_mut(&plain_city).unwrap().wall_hp = 400;
    // The rich target outscores the plain one on garrison — but a scout
    // from a power we are at peace with stands in its blast.
    let ring: Vec<Pos> =
        game.wdisk(rich, radius)
            .into_iter()
            .filter(|position| {
                *position != rich
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
                    && game.unit_ids_at(*position).is_empty()
            })
            .take(4)
            .collect();
    assert!(ring.len() == 4, "blast ring has four open land tiles");
    for post in &ring[..3] {
        game.spawn_test_unit("warrior", 1, *post);
    }
    game.spawn_test_unit("scout", 2, ring[3]);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(rich_city),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.advanced_wmd_strikes(&mut game, 0, &plan);
    assert_eq!(
        game.players[0].counters["project_effect:thermonuclear_devices"], 0,
        "the device is still spent — on the clean target"
    );
    assert!(
        game.map.tiles[&rich].fallout_until <= game.turn,
        "the bystander's post holds the strike off the richer city"
    );
    assert!(
        game.map.tiles[&plain].fallout_until > game.turn,
        "the quieter city takes the strike instead"
    );
}

#[test]
fn targeted_science_recovers_a_persistent_idle_city_with_a_civilian() {
    let (mut game, city, _) = empire_with_a_capital(79_123);
    clear_barbarian_fixture(&mut game);
    game.at_war.clear();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    // Make every ordinary candidate miss the -1,000 choice bar while leaving
    // the Builder as a real, non-military fallback above the -9,000 hard veto.
    // This is the live failure shape: a soft refusal is still buildable, but
    // the old branch discarded it and spent the turn idle.
    ai.base.w.p_builder = -200.0;
    ai.base.w.p_military = -200.0;
    let counts = ai.counts(&game, 0);
    let items = game.producible_items(0, city);
    let scores = ai.production_values(&game, 0, city, &items, &plan, counts);
    assert!(items.iter().any(|item| matches!(
        item,
        Item::Unit { unit } if unit == "builder"
    )));
    assert!(scores.iter().all(|score| *score <= -1_000.0));
    assert!(scores.iter().all(|score| *score > PRODUCTION_VETO_FLOOR));

    ai.advanced_production(&mut game, 0, &plan, false);
    assert!(
        game.cities[&city].queue.is_empty(),
        "one transient idle turn remains tolerated"
    );

    // Repeating the same host turn is common while the bridge drains a
    // blocking prompt and must not count as a second idle turn.
    ai.advanced_production(&mut game, 0, &plan, false);
    assert!(game.cities[&city].queue.is_empty());

    game.turn += 1;
    ai.advanced_production(&mut game, 0, &plan, false);
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Unit { unit }) if unit == "builder"
    ));
}

/// A pending promotion blocks further XP, including on a healthy ranged
/// unit that might never need the promotion's healing.
#[test]
fn pending_promotions_do_not_wait_for_damage() {
    for (kind, hp) in [("warrior", 100), ("archer", 100), ("warrior", 40)] {
        let (mut game, _capital, home) = empire_with_a_capital(71_107);
        let uid = game.spawn_test_unit(kind, 0, anchor_at(&game, home, 1));
        let unit = game.units.get_mut(&uid).unwrap();
        unit.xp = 15;
        unit.level = 1;
        unit.hp = hp;
        unit.moves_left = 2.0;
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_promote_when_wounded();
        assert!(game.promotion_pending(uid));
        assert!(!game.available_promotions(uid).is_empty());
        ai.advanced_promotions(&mut game, 0, GrandStrategy::Conquest);
        assert_eq!(game.units[&uid].promotions.len(), 1);
        assert_eq!(game.units[&uid].level, 2);
        assert!(!game.promotion_pending(uid), "XP awards must be unblocked");
        assert_eq!(game.units[&uid].hp, (hp + 50).min(100));
        assert_eq!(game.units[&uid].xp, 15);

        // A later earned promotion is also taken promptly.
        let unit = game.units.get_mut(&uid).unwrap();
        unit.xp = 45;
        unit.moves_left = 2.0;
        ai.advanced_promotions(&mut game, 0, GrandStrategy::Conquest);
        assert_eq!(game.units[&uid].promotions.len(), 2);
        assert!(!game.promotion_pending(uid));
    }
}

#[test]
fn exact_hybrid_search_uses_melee_to_finish_a_city() {
    let mut game = Game::new_full(2, 24, 16, 71_010, 120, 0, false);
    let rival_origin = game
        .player_unit_ids(1)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .map(|unit| game.units[&unit].pos)
        .unwrap();
    let city = game.found_city_for(1, rival_origin, None);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let target = game.cities[&city].pos;
    let staging = game
        .nbrs(target)
        .into_iter()
        .find(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
                && game.unit_ids_at(*position).is_empty()
        })
        .unwrap();
    game.cities.get_mut(&city).unwrap().hp = 0;
    game.cities.get_mut(&city).unwrap().wall_hp = 0;
    let robot = game.spawn_test_unit("giant_death_robot", 0, staging);
    game.at_war.insert((0, 1));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(city),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);

    assert!(ai.advanced_military_step(&mut game, 0, robot, &plan));
    assert_eq!(game.cities[&city].owner, 0);
    assert!(matches!(
        game.log.last(),
        Some((0, Action::Attack { unit, target: action_target }))
            if *unit == robot && *action_target == target
    ));
}

#[test]
fn bounded_purchase_menu_workers_keep_four_city_order_exact() {
    let mut game = Game::new_full(1, 40, 24, 7_106_002, 160, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let first = game.player_city_ids(0)[0];
    let anchor = game.cities[&first].pos;
    for _ in 0..3 {
        found_nearby_test_city(&mut game, 0, anchor);
    }
    game.players[0].gold = 5_000.0;

    let serial = AdvancedAi::targeting(VictoryTarget::Science);
    let serial_actions = serial.legal_purchase_actions(&game, 0);
    assert!(!serial_actions.is_empty());

    let mut parallel = serial.clone();
    parallel.work_pool = Some(Arc::new(WorkPool::new(PURCHASE_MENU_MAX_WORKERS + 1)));
    assert_eq!(parallel.legal_purchase_actions(&game, 0), serial_actions);
}

#[test]
fn culture_builders_upgrade_farms_to_resorts_without_reverting_them() {
    let mut g = Game::new(2, 24, 16, 74, 80, 0);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.players[0].techs.insert(crate::name!("radio"));
    let city = g.player_city_ids(0)[0];
    let pos = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != g.cities[&city].pos)
        .unwrap();
    let tile = g.map.tiles.get_mut(&pos).unwrap();
    tile.terrain = crate::name!("plains");
    tile.feature = None;
    tile.resource = None;
    tile.hills = false;
    tile.improvement = Some(crate::name!("farm"));
    for neighbor in g.nbrs(pos) {
        let tile = g.map.tiles.get_mut(&neighbor).unwrap();
        tile.terrain = crate::name!("coast");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
    }
    assert!(g.tile_appeal(pos) >= 4);

    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    let upgrades = ai.worthwhile_improvements(&g, 0, pos, GrandStrategy::Culture);
    assert_eq!(
        upgrades.first().map(|name| name.as_str()),
        Some("seaside_resort")
    );

    g.map.tiles.get_mut(&pos).unwrap().improvement = Some(crate::name!("seaside_resort"));
    assert!(ai
        .worthwhile_improvements(&g, 0, pos, GrandStrategy::Culture)
        .is_empty());
}

#[test]
fn delegated_domination_does_not_start_the_baseline_space_race() {
    let mut game = Game::new_full(
        1,
        30,
        20,
        crate::rng::fixture_seed("SPACEPORT", 324_006),
        650,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    game.cities.get_mut(&city).unwrap().pop = 10;
    game.players[0].techs.insert(crate::name!("rocketry"));
    game.players[0].gold = 1_000.0;
    game.players[0].gold_per_turn = 100.0;
    game.turn = 170;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.book_pos = 4;
    ai.base.w.mil_per_city = 0.0;
    ai.base.w.city_target = 1.0;
    ai.base.w.settler_stop_turn = 0.0;
    ai.base.recon_replacement = false;
    assert!(!game
        .district_sites(city, crate::name!("spaceport"))
        .is_empty());
    let mut control = game.clone();
    ai.base.cities(&mut control, 0);
    assert!(
        matches!(
            control.cities[&city].queue.first(),
            Some(Item::District { district, .. }) if district == "spaceport"
        ),
        "baseline choice: {:?}",
        control.cities[&city].queue.first()
    );

    ai.delegated_cities(&mut game, 0, &plan);
    assert!(
        !matches!(game.cities[&city].queue.first(), Some(Item::District { district, .. }) if district == "spaceport"),
        "the domination assignment must reach the delegated governor"
    );
    assert!(
        !ai.base.exclude_space_race,
        "the call-local gate must be restored"
    );
}

#[test]
fn envoy_dividends_amani_and_league_match_actual_placements() {
    for puppeteer in [false, true] {
        let (mut g, states) = envoy_dividend_board(1);
        let minor = states[0];
        let city = g.player_city_ids(minor)[0];
        g.turn = 100;
        g.players[0].governor_roster.insert(
            "amani".to_string(),
            crate::game::GovernorState {
                city: Some(city),
                assigned_turn: 0,
                disabled_until: 0,
                promotions: if puppeteer {
                    ["puppeteer".to_string()].into_iter().collect()
                } else {
                    Default::default()
                },
            },
        );
        g.players[0].policies.insert(Name::new("diplomatic_league"));
        g.players[1].envoys = vec![(minor, 10)];
        let options = g.envoy_investment_options(0, minor);
        assert!(!options.is_empty());
        for (cost, projected, _) in options {
            let mut actual = g.clone();
            for _ in 0..cost {
                actual
                    .apply(0, &Action::SendEnvoy { player: minor })
                    .unwrap();
            }
            assert_eq!(actual.envoys_at(0, minor), projected);
        }
    }
}

#[test]
fn hostile_memory_v2_does_not_apply_the_land_guard_rule_to_a_ship() {
    let (mut game, mut ai, _, settler, _, water) = remembered_naval_escort_field();
    ai.enable_hostile_memory_2();
    let ship = game.spawn_test_unit("galley", 0, water);
    let target = game.units[&settler].pos;
    assert_eq!(
        ai.escort_naval_memory_step(&mut game, 0, ship, target),
        None
    );
}

#[test]
fn selected_wounded_galley_is_reserved_before_live_finishing() {
    let (g, mut ai, ours, _, _) = wounded_galley_prepass_fixture();
    assert!(ai.live_wounded_unit_reservations(&g, 0).is_empty());
    ai.enable_wounded_out_of_reach();
    assert!(ai.live_wounded_unit_reservations(&g, 0).contains(&ours));
    assert_eq!(g.units[&ours].hp, 40);
}
