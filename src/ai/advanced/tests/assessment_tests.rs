//! Tests of `advanced/assessment.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

/// An assigned Science lane may use the first few turns to establish a small
/// empire, but it must not inherit the live seat's sixteen-city land grab.
/// Once the opening window closes, the plan must become Science even when a
/// safe site remains, so Campus buildings and the space-race funnel can own
/// production before the generic expansion deadline.
#[test]
fn a_science_target_caps_land_grab_and_takes_over_after_the_opening() {
    let mut game = Game::new_with(GameOptions {
        speed: "online".to_string(),
        ..GameOptions::new(2, 74, 46, 91_508, 250, 0)
    });
    game.victory_conditions = crate::game::VictoryConditions::parse("science,score").unwrap();
    game.current = 0;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.found_city_for(0, game.units[&settler].pos, None);
    game.remove_unit(settler);

    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_live_bridge_universe();

    game.turn = 1;
    let opening = ai.assess(&game, 0);
    assert_eq!(
        opening.desired_cities, SHARED_CITY_TARGET_CEILING,
        "the first half must keep the full expansion horizon"
    );
    assert_eq!(
        opening.strategy,
        GrandStrategy::Expansion,
        "the target may still establish its first city opening"
    );

    // An assigned Science lane stays in the expansion posture after two
    // cities while the first half is still running. This catches the
    // hostile-frontier case where the old handoff stranded a third Settler
    // while the target lane started tailoring too early.
    let first_city = game.player_city_ids(0)[0];
    let second_position = game
        .map
        .tiles
        .iter()
        .find(|(position, tile)| {
            tile.owner_city.is_none()
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.wdist(**position, game.cities[&first_city].pos) >= 7
        })
        .map(|(position, _)| *position)
        .unwrap();
    game.found_city_for(0, second_position, None);
    let two_city_handoff = ai.assess(&game, 0);
    assert_eq!(
        two_city_handoff.strategy,
        GrandStrategy::Expansion,
        "the first half keeps expanding before Science takes over"
    );

    game.turn = game.standard_duration(60);
    let handoff = ai.assess(&game, 0);
    assert_eq!(handoff.desired_cities, SHARED_CITY_TARGET_CEILING);
    assert_eq!(
        handoff.strategy,
        GrandStrategy::Expansion,
        "the assigned lane stays expansion-first through the first half"
    );

    game.turn = game.max_turns / 2;
    let specialized = ai.assess(&game, 0);
    assert_eq!(specialized.desired_cities, SHARED_SCIENCE_CITY_TARGET);
    assert_eq!(
        specialized.strategy,
        GrandStrategy::Science,
        "the victory lane takes over at halfway"
    );
}

/// `science-opening-band` remains compatible with the general phase policy:
/// the lane is still Expansion in the first half, and the victory plan takes
/// over at the shared halfway boundary rather than at the old band deadline.
#[test]
fn the_opening_band_keeps_a_two_city_science_seat_expanding() {
    let mut game = Game::new_with(GameOptions {
        speed: "online".to_string(),
        ..GameOptions::new(2, 74, 46, 91_508, 250, 0)
    });
    game.victory_conditions = crate::game::VictoryConditions::parse("science,score").unwrap();
    game.current = 0;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.found_city_for(0, game.units[&settler].pos, None);
    game.remove_unit(settler);
    let first_city = game.player_city_ids(0)[0];
    let second_position = game
        .map
        .tiles
        .iter()
        .find(|(position, tile)| {
            tile.owner_city.is_none()
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.wdist(**position, game.cities[&first_city].pos) >= 7
        })
        .map(|(position, _)| *position)
        .unwrap();
    game.found_city_for(0, second_position, None);

    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_live_bridge_universe();
    ai.enable_science_opening_band();

    game.turn = 1;
    let inside_band = ai.assess(&game, 0);
    assert_eq!(
        inside_band.strategy,
        GrandStrategy::Expansion,
        "inside the band two cities must not end the Science opening"
    );

    game.turn = game.standard_duration(100);
    let window_closed = ai.assess(&game, 0);
    assert_eq!(
        window_closed.strategy,
        GrandStrategy::Expansion,
        "the old band deadline must not end the first-half expansion posture"
    );

    game.turn = game.max_turns / 2;
    assert_eq!(
        ai.assess(&game, 0).strategy,
        GrandStrategy::Science,
        "the assigned lane takes over at halfway"
    );
}

#[test]
fn an_explicit_science_target_keeps_its_lane_against_a_power_gap() {
    let (mut game, _) = outgunned_at_war_fixture();
    game.turn = 100;

    let science = AdvancedAi::targeting(VictoryTarget::Science);
    let plan = science.assess(&game, 0);

    assert_eq!(
        plan.strategy,
        GrandStrategy::Science,
        "a Science contract must not become permanent Recovery merely because
         a wartime rival has more military power"
    );
    assert!(
        plan.threatened_city.is_none(),
        "the fixture must contain no local city threat"
    );
}

#[test]
fn a_requested_city_state_campaign_preserves_its_suzerain() {
    let mut game = Game::new_full(2, 30, 18, 711, 300, 1, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    game.turn = 200;
    let minor = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .unwrap()
        .id;
    game.record_contact(0, 1);
    game.record_contact(0, minor);
    let rival_capital = game.cities[&game.player_city_ids(1)[0]].pos;
    for _ in 0..6 {
        game.spawn_test_unit("giant_death_robot", 1, rival_capital);
    }

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    // A requested city-state front can override capital priority, but it
    // still cannot turn our own suzerained city-state into a legal target.
    ai.forced_target_player = Some(minor);
    let exposed = ai.assess(&game, 0);
    assert_eq!(exposed.strategy, GrandStrategy::Conquest);
    assert_eq!(exposed.target_player, Some(minor));

    game.players[0].envoys = vec![(minor, 3)];
    assert_eq!(game.suzerain_of(minor), Some(0));
    let allied = ai.assess(&game, 0);
    assert_eq!(allied.target_player, Some(1));
}

#[test]
fn live_campaign_holds_a_major_war_city_until_capture_or_emergency() {
    let mut game = Game::new_full(2, 30, 18, 7_112, 300, 0, false);
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
    let enemy_capital = game.player_city_ids(1)[0];
    let enemy_position = game.cities[&enemy_capital].pos;
    let outpost_position = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && tile.owner_city.is_none()
                && game.wdist(enemy_position, **position) >= 9
                && game
                    .cities
                    .values()
                    .all(|city| game.wdist(city.pos, **position) >= 4)
        })
        .map(|(position, _)| *position)
        .next()
        .expect("test map has a second enemy-city site");
    game.current = 1;
    let settler = game.spawn_test_unit("settler", 1, outpost_position);
    game.apply(1, &Action::FoundCity { unit: settler })
        .expect("the enemy can found its outpost");
    let enemy_outpost = game
        .player_city_ids(1)
        .into_iter()
        .find(|city| *city != enemy_capital)
        .expect("the enemy owns two cities");
    game.current = 0;
    game.at_war.insert((0, 1));
    game.spawn_test_unit("scout", 0, game.cities[&enemy_capital].pos);
    game.spawn_test_unit("scout", 0, game.cities[&enemy_outpost].pos);

    // Let the regular ranking choose the city it would otherwise re-pick
    // on the next assessment, then preserve the other existing objective.
    let mut ranking = AdvancedAi::targeting(VictoryTarget::Domination);
    ranking.belief.observe(&game, 0);
    let ranked = ranking.assess(&game, 0);
    assert_eq!(ranked.target_player, Some(1));
    let ranked_target = ranked
        .target_city
        .expect("the major war has a city objective");
    let previous_target = [enemy_capital, enemy_outpost]
        .into_iter()
        .find(|city| *city != ranked_target)
        .expect("the enemy's other city is a valid previous objective");
    let prior_plan = StrategicPlan {
        strategy: ranked.strategy,
        target_player: Some(1),
        target_city: Some(previous_target),
        threatened_city: None,
        desired_cities: ranked.desired_cities,
        assessed_turn: game.turn.saturating_sub(1),
        rush: false,
    };

    let mut ordinary = AdvancedAi::targeting(VictoryTarget::Domination);
    ordinary.belief.observe(&game, 0);
    ordinary.plan = Some(prior_plan.clone());
    assert_eq!(
        ordinary.assess(&game, 0).target_city,
        Some(ranked_target),
        "the frozen planner still refreshes its objective from the city ranking"
    );

    let mut live = AdvancedAi::targeting(VictoryTarget::Domination);
    live.enable_siege_commitment();
    live.belief.observe(&game, 0);
    live.plan = Some(prior_plan);
    assert_eq!(
        live.assess(&game, 0).target_city,
        Some(previous_target),
        "the live major-war column keeps marching toward the city it was ordered to take"
    );

    // A live emergency is more urgent than the offensive campaign, even
    // if it names the city that the ordinary ranking would have picked.
    game.active_emergencies.push(crate::game::Emergency {
        id: 1,
        kind: "military".to_string(),
        target: 1,
        city: ranked_target,
        original_owner: 1,
        members: [0].into_iter().collect(),
        contributions: BTreeMap::new(),
        started: game.turn,
        ends: game.turn + 30,
    });
    assert_eq!(
        live.assess(&game, 0).target_city,
        Some(ranked_target),
        "an emergency may immediately override the committed campaign city"
    );
    game.active_emergencies.clear();

    // Capture invalidates the old objective. The remaining enemy city is
    // then eligible for a fresh campaign ranking immediately.
    game.cities.get_mut(&previous_target).unwrap().owner = 0;
    assert_eq!(
        live.assess(&game, 0).target_city,
        Some(ranked_target),
        "a captured city cannot keep the campaign target pinned"
    );
}

#[test]
fn domination_column_takes_a_damaged_open_foothold_before_a_fresh_walled_target() {
    let mut game = Game::new_full(2, 30, 18, 7_113, 300, 0, false);
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
    let walled = game.player_city_ids(1)[0];
    let outpost_pos = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && tile.owner_city.is_none()
                && game.wdist(game.cities[&walled].pos, **pos) >= 4
                && game
                    .cities
                    .values()
                    .all(|city| game.wdist(city.pos, **pos) >= 4)
        })
        .map(|(pos, _)| *pos)
        .next()
        .unwrap();
    game.current = 1;
    let settler = game.spawn_test_unit("settler", 1, outpost_pos);
    game.apply(1, &Action::FoundCity { unit: settler }).unwrap();
    let open = game
        .player_city_ids(1)
        .into_iter()
        .find(|city| *city != walled)
        .unwrap();
    game.current = 0;
    game.turn = 115;
    for pos in game.map.tiles.keys().copied().collect::<Vec<_>>() {
        if game.player_city_ids(0).len() >= 6 {
            break;
        }
        if game
            .map
            .get(pos)
            .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
            && game
                .cities
                .values()
                .all(|city| game.wdist(city.pos, pos) >= 4)
        {
            game.found_city_for(0, pos, None);
        }
    }
    assert_eq!(game.player_city_ids(0).len(), 6);
    game.at_war.insert((0, 1));
    game.spawn_test_unit("scout", 0, game.cities[&open].pos);
    game.spawn_test_unit("scout", 0, game.cities[&walled].pos);
    game.cities
        .get_mut(&walled)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    game.cities.get_mut(&walled).unwrap().wall_hp = 100;

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_siege_commitment();
    let threatened_city = ai.threatened_city(&game, 0);
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(walled),
        threatened_city,
        desired_cities: 4,
        assessed_turn: game.turn.saturating_sub(1),
        rush: false,
    });
    ai.belief.observe(&game, 0);
    assert!(!ai.plan_stale(&game, 0));
    assert_eq!(ai.assess(&game, 0).target_city, Some(walled));

    let taker_pos = game
        .nbrs(game.cities[&open].pos)
        .into_iter()
        .find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        })
        .unwrap();
    let mut fresh = game.clone();
    let mut early = ai.clone();
    fresh.spawn_test_unit("heavy_chariot", 0, taker_pos);
    early.belief.observe(&fresh, 0);
    assert!(
        early.plan_stale(&fresh, 0),
        "a healthy early taker can seize a weaker open foothold before it builds Walls"
    );
    assert_eq!(early.assess(&fresh, 0).target_city, Some(open));

    game.cities.get_mut(&open).unwrap().hp = 175;
    assert!(
        !ai.plan_stale(&game, 0),
        "a foothold without a nearby land taker does not interrupt the plan"
    );
    assert_eq!(
        ai.assess(&game, 0).target_city,
        Some(walled),
        "damage without a nearby land taker is no reason to redirect"
    );
    game.spawn_test_unit("man_at_arms", 0, taker_pos);
    ai.belief.observe(&game, 0);
    assert!(
        ai.plan_stale(&game, 0),
        "the open foothold interrupts the five-turn planning cadence"
    );
    let opportunity = ai.assess(&game, 0);
    assert_eq!(opportunity.strategy, GrandStrategy::Conquest);
    assert_eq!(
        opportunity.target_city,
        Some(open),
        "a nearby melee column should finish its damaged open foothold"
    );

    game.cities.get_mut(&walled).unwrap().wall_hp = 60;
    assert_eq!(
        ai.assess(&game, 0).target_city,
        Some(walled),
        "an already breached objective keeps its own invested assault"
    );
}
