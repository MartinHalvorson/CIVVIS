use super::*;

fn bankrupt_campaign() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut game = Game::new_full(2, 74, 46, 79_101, 200, 0, false);
    game.current = 0;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let home = game.cities[&city].pos;
    let other = *game
        .map
        .tiles
        .iter()
        .find(|(pos, tile)| {
            tile.owner_city.is_none()
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && (4..=10).contains(&game.wdist(home, **pos))
        })
        .unwrap()
        .0;
    game.found_city_for(0, other, None);
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let rival_settler = game
        .player_unit_ids(1)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let target_city = game.found_city_for(1, game.units[&rival_settler].pos, None);
    game.turn = 150;
    game.at_war.clear();
    game.players[0].techs.insert(crate::name!("archery"));
    game.players[0].civics.insert(crate::name!("foreign_trade"));
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -18.0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.disable_war_economy();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target_city),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    (game, ai, plan, city)
}

#[test]
fn bankrupt_domination_production_funds_the_army_before_adding_upkeep() {
    let (mut game, mut ai, plan, city) = bankrupt_campaign();
    ai.advanced_production(&mut game, 0, &plan, false);
    assert_eq!(
        game.cities[&city].queue.first(),
        Some(&Item::Unit {
            unit: crate::name!("trader"),
        }),
        "a committed campaign needs income to keep its army"
    );

    // Once the route slot is occupied, recover recurring income from the
    // already-built trade district instead of extending the military queue.
    let (mut game, mut ai, plan, city) = bankrupt_campaign();
    let home = game.cities[&city].pos;
    game.spawn_test_unit("trader", 0, home);
    game.players[0].techs.insert(crate::name!("currency"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("commercial_hub"), home);
    let market = Item::Building {
        building: crate::name!("market"),
    };
    assert!(game.can_produce(0, city, &market));
    ai.advanced_production(&mut game, 0, &plan, false);
    assert_eq!(game.cities[&city].queue.first(), Some(&market));
}

#[test]
fn domination_recovery_preserves_solvency_and_emergency_defense_exceptions() {
    let (mut game, ai, _, _) = bankrupt_campaign();
    let recovering = |g: &Game| ai.live_war_economy_requires_recovery(g, 0, &ai.counts(g, 0));
    assert!(recovering(&game));
    game.at_war.insert((0, 1));
    assert!(
        !recovering(&game),
        "a missing wartime garrison remains urgent"
    );
    game.at_war.clear();
    game.players[0].gold_per_turn = 1.0;
    assert!(!recovering(&game));
    game.players[0].gold_per_turn = -18.0;
    game.players[0].gold = 150.0;
    assert!(!recovering(&game));
    game.players[0].gold = 0.0;
    game.victory_conditions.domination = false;
    assert!(!recovering(&game));
}
