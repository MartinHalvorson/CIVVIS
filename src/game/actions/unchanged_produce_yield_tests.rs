//! Repeated production keeps answers only after validation and post-effects.

use super::*;

fn capitals() -> (Game, u32, u32, Action) {
    let mut game = Game::new_full(2, 24, 16, 91_171, 100, 0, false);
    for pid in 0..2 {
        while game.current != pid {
            game.apply(game.current, &Action::EndTurn).unwrap();
        }
        let unit = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit }).unwrap();
    }
    while game.current != 0 {
        game.apply(game.current, &Action::EndTurn).unwrap();
    }
    let ours = game.player_city_ids(0)[0];
    let theirs = game.player_city_ids(1)[0];
    let action = Action::Produce {
        city: ours,
        item: Item::Unit {
            unit: crate::name!("warrior"),
        },
    };
    game.apply(0, &action).unwrap();
    game.keep_rival_yields(true);
    game.keep_own_yields(true);
    (game, ours, theirs, action)
}

fn warm(game: &Game, ours: u32, theirs: u32) {
    game.city_yields(ours);
    game.city_yields(theirs);
    assert!(game.query_memo.own_yields.borrow().contains_key(&ours));
    assert!(game.query_memo.rival_yields.borrow().contains_key(&theirs));
}

fn bits(yields: Yields) -> [u64; 6] {
    [
        yields.food,
        yields.production,
        yields.gold,
        yields.science,
        yields.culture,
        yields.faith,
    ]
    .map(f64::to_bits)
}

fn same_world_and_yields(game: &Game, reference: &Game, cities: [u32; 2]) {
    assert_eq!(
        serde_json::to_value(game).unwrap(),
        serde_json::to_value(reference).unwrap(),
        "the complete serialized world, including action logging, is unchanged"
    );
    for city in cities {
        assert_eq!(
            bits(game.city_yields(city)),
            bits(reference.city_yields(city))
        );
    }
}

#[test]
fn accepted_identical_production_reuses_own_and_rival_answers() {
    let (mut game, ours, theirs, action) = capitals();
    game.audit_rival_yields(true);
    warm(&game, ours, theirs);
    let before = game.rival_yields_reused();
    let mut reference = game.clone();
    reference.apply(0, &action).unwrap();
    game.apply(0, &action).unwrap();
    assert!(game.query_memo.own_yields.borrow().contains_key(&ours));
    assert!(game.query_memo.rival_yields.borrow().contains_key(&theirs));
    same_world_and_yields(&game, &reference, [ours, theirs]);
    assert_eq!(game.rival_yields_reused(), (before.0 + 2, before.1));
}

#[test]
fn identical_but_illegal_production_still_refuses_and_drops_answers() {
    let (mut game, ours, theirs, _) = capitals();
    let item = Item::Building {
        building: crate::name!("monument"),
    };
    game.cities
        .get_mut(&ours)
        .unwrap()
        .buildings
        .insert(crate::name!("monument"));
    assert!(!game.can_produce(0, ours, &item));
    game.cities.get_mut(&ours).unwrap().queue = vec![item.clone()];
    warm(&game, ours, theirs);
    let action = Action::Produce { city: ours, item };
    let mut reference = game.clone();
    let expected = reference.apply(0, &action).unwrap_err();
    assert_eq!(game.apply(0, &action).unwrap_err(), expected);
    assert!(game.query_memo.own_yields.borrow().is_empty());
    assert!(game.query_memo.rival_yields.borrow().is_empty());
    same_world_and_yields(&game, &reference, [ours, theirs]);
}

#[test]
fn a_different_queue_still_invalidates_our_cities() {
    let (mut game, ours, theirs, _) = capitals();
    let item = game
        .producible_items(0, ours)
        .into_iter()
        .find(|item| {
            !matches!(item, Item::District { .. }) && game.cities[&ours].queue.first() != Some(item)
        })
        .expect("a capital has another legal production item");
    warm(&game, ours, theirs);
    let action = Action::Produce { city: ours, item };
    let mut reference = game.clone();
    reference.apply(0, &action).unwrap();
    game.apply(0, &action).unwrap();
    assert!(game.query_memo.own_yields.borrow().is_empty());
    same_world_and_yields(&game, &reference, [ours, theirs]);
}

#[test]
fn identical_district_requests_still_place_the_foundation() {
    let (mut game, ours, theirs, _) = capitals();
    game.players[0].techs.insert(crate::name!("writing"));
    let item = game
        .producible_items(0, ours)
        .into_iter()
        .find(|item| matches!(item, Item::District { .. }))
        .expect("Writing opens a district site");
    game.cities.get_mut(&ours).unwrap().queue = vec![item.clone()];
    game.board_written();
    warm(&game, ours, theirs);
    let epoch = game.map.tiles.epoch();
    let action = Action::Produce { city: ours, item };
    let mut reference = game.clone();
    reference.apply(0, &action).unwrap();
    game.apply(0, &action).unwrap();
    assert_ne!(game.map.tiles.epoch(), epoch);
    assert!(game.query_memo.own_yields.borrow().is_empty());
    same_world_and_yields(&game, &reference, [ours, theirs]);
}

#[test]
fn post_action_contact_discovery_invalidates_retained_answers() {
    let (mut game, ours, theirs, action) = capitals();
    let unit = game.player_unit_ids(0)[0];
    game.units.get_mut(&unit).unwrap().pos = game.cities[&theirs].pos;
    game.players[0].met.remove(&1);
    game.players[1].met.remove(&0);
    game.board_written();
    warm(&game, ours, theirs);
    assert!(!game.players[0].met.contains(&1));
    let mut reference = game.clone();
    reference.apply(0, &action).unwrap();
    game.apply(0, &action).unwrap();
    assert!(game.players[0].met.contains(&1));
    assert!(game.query_memo.own_yields.borrow().is_empty());
    assert!(game.query_memo.rival_yields.borrow().is_empty());
    same_world_and_yields(&game, &reference, [ours, theirs]);
}

#[test]
fn post_action_wonder_discovery_is_not_skipped_or_cached_stale() {
    let (mut game, ours, theirs, action) = capitals();
    let wonder = *game
        .rules
        .features
        .iter()
        .find(|(name, spec)| {
            spec.natural_wonder && !game.players[0].discovered_natural_wonders.contains(*name)
        })
        .expect("the catalogue has an undiscovered natural wonder")
        .0;
    let position = game.nbrs(game.cities[&ours].pos)[0];
    game.map.tiles.get_mut(&position).unwrap().feature = Some(wonder);
    game.players[0].explored.remove(&position);
    game.board_written();
    warm(&game, ours, theirs);
    let mut reference = game.clone();
    reference.apply(0, &action).unwrap();
    game.apply(0, &action).unwrap();
    assert!(game.players[0].discovered_natural_wonders.contains(&wonder));
    assert!(game.query_memo.own_yields.borrow().is_empty());
    assert!(game.query_memo.rival_yields.borrow().is_empty());
    same_world_and_yields(&game, &reference, [ours, theirs]);
}

#[test]
fn post_action_monopoly_moments_are_not_skipped_or_cached_stale() {
    let (mut game, ours, theirs, action) = capitals();
    for tile in game.map.tiles.values_mut() {
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    let center = game.cities[&ours].pos;
    let positions: Vec<Pos> = game.cities[&ours]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .take(3)
        .collect();
    assert_eq!(positions.len(), 3);
    for position in positions {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = Some(crate::name!("silk"));
        tile.improvement = Some(crate::name!("plantation"));
    }
    game.players[0].techs.insert(crate::name!("currency"));
    for player in game.players.iter_mut() {
        player
            .counters
            .remove("historic_moment:first_luxury_monopoly");
    }
    game.players[0].era_score = 0;
    game.query_memo.monopoly_context.borrow_mut().take();
    game.board_written();
    assert_eq!(game.controlled_resource_count(0, "silk"), 3);
    warm(&game, ours, theirs);
    let mut reference = game.clone();
    reference.apply(0, &action).unwrap();
    game.apply(0, &action).unwrap();
    assert_eq!(
        game.players[0]
            .counters
            .get("historic_moment:first_luxury_monopoly"),
        Some(&1)
    );
    assert!(game.players[0].era_score > 0);
    assert!(game.query_memo.own_yields.borrow().is_empty());
    assert!(game.query_memo.rival_yields.borrow().is_empty());
    same_world_and_yields(&game, &reference, [ours, theirs]);
}

#[test]
fn policy_tracing_still_derives_after_a_repeated_production_request() {
    let (mut game, ours, theirs, action) = capitals();
    warm(&game, ours, theirs);
    game.apply(0, &action).unwrap();
    let before = game.rival_yields_reused();
    let trace = game.trace_policy_reads(0);
    game.city_yields(ours);
    assert_eq!(game.rival_yields_reused(), before);
    drop(trace);
    game.city_yields(ours);
    assert_eq!(game.rival_yields_reused(), (before.0 + 1, before.1));
}

#[test]
fn provisionally_retained_answers_never_answer_while_an_action_is_in_flight() {
    let (mut game, ours, theirs, _) = capitals();
    warm(&game, ours, theirs);
    let old = game.city_yields(ours);
    let before = game.rival_yields_reused();
    game.query_memo.action_begins(ActionReach::Unchanged);
    let population = game.cities[&ours].pop;
    game.cities.get_mut(&ours).unwrap().pop += 3;
    assert_ne!(bits(game.city_yields(ours)), bits(old));
    assert_eq!(game.rival_yields_reused(), before);
    game.cities.get_mut(&ours).unwrap().pop = population;
    game.query_memo.action_ends(ActionReach::Unchanged);
    assert_eq!(bits(game.city_yields(ours)), bits(old));
    assert_eq!(game.rival_yields_reused(), (before.0 + 1, before.1));
}
