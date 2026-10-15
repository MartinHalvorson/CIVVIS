use super::*;

fn watched_frontier() -> (Game, u32) {
    let mut game = Game::new_full(2, 48, 24, 4063, 650, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.clear_mirror_cities();
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let capital = game.found_city_for(0, (12, 10), None);
    let frontier = game.found_city_for(0, (24, 10), None);
    game.found_city_for(1, (35, 10), None);
    assert!(game.cities[&capital].is_capital);
    assert!(!game.cities[&frontier].is_capital);
    game.cities.get_mut(&frontier).unwrap().pop = 4;
    game.current = 0;
    game.turn = 30;
    game.record_contact(0, 1);
    game.players[0].techs.insert(crate::name!("mining"));
    Arc::make_mut(&mut game.observed_military_power).insert(0, 100.0);
    Arc::make_mut(&mut game.observed_military_power).insert(1, 250.0);
    let soldier = (31, 10);
    assert_eq!(game.wdist(game.cities[&frontier].pos, soldier), 7);
    assert!(game.wdist(game.cities[&capital].pos, soldier) > 8);
    game.spawn_test_unit("warrior", 1, soldier);
    game.host_observed = Arc::new(game.wdisk((24, 10), 8).into_iter().collect());
    (game, frontier)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_capital_walls_before_the_surprise();
    ai.enable_frontier_walls_before_the_surprise();
    ai
}

fn walls() -> Item {
    Item::Building {
        building: crate::name!("walls"),
    }
}

#[test]
fn a_watched_frontier_unlocks_masonry_before_a_peaceful_rival_declares() {
    let (game, _) = watched_frontier();
    let mut capital_only = AdvancedAi::new();
    capital_only.enable_capital_walls_before_the_surprise();
    capital_only.observe_turn_start_hostiles(&game, 0);
    assert!(capital_only
        .capital_walls_research_threat(&game, 0)
        .is_none());
    assert!(capital_only
        .frontier_walls_research_goal(&game, 0)
        .is_none());

    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    assert_eq!(
        ai.frontier_walls_research_goal(&game, 0),
        Some(crate::name!("masonry")),
        "the original capital's distant warning cannot prepare this frontier"
    );
}

#[test]
fn the_watched_frontier_claims_legal_walls_before_routine_production() {
    let (mut game, frontier) = watched_frontier();
    game.players[0].techs.insert(crate::name!("masonry"));
    assert!(game.can_produce(0, frontier, &walls()));
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    game.apply(
        0,
        &Action::Produce {
            city: frontier,
            item: builder.clone(),
        },
    )
    .unwrap();
    let mut off = game.clone();
    AdvancedAi::new().claim_frontier_walls_before_the_surprise(&mut off, 0);
    assert_eq!(off.cities[&frontier].queue.first(), Some(&builder));

    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    ai.claim_frontier_walls_before_the_surprise(&mut game, 0);
    assert_eq!(
        game.cities[&frontier].queue.first(),
        Some(&walls()),
        "an available first wall is prepared before the surprise declaration"
    );
}
