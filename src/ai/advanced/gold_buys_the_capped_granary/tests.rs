use super::*;

/// A founded capital with Pottery, its population raised to its housing, a
/// rich treasury and an idle queue.
fn capped_capital() -> (Game, u32) {
    let mut game = Game::new_full(1, 24, 16, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("pottery"));
    let housing = game.city_housing(&game.cities[&capital]);
    game.cities.get_mut(&capital).unwrap().pop = housing.floor() as i32;
    game.cities.get_mut(&capital).unwrap().queue.clear();
    game.players[0].gold = 1_000.0;
    (game, capital)
}

fn plan_for(game: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    }
}

fn has_granary(game: &Game, cid: u32) -> bool {
    game.cities[&cid]
        .buildings
        .iter()
        .any(|building| *building == "granary")
}

/// Off, nothing is bought. On, the capped capital buys its Granary, and a
/// second pass buys nothing more.
#[test]
fn a_capped_city_buys_its_granary_under_the_gene() {
    let (mut off, capital) = capped_capital();
    let plan = plan_for(&off);
    assert!(!AdvancedAi::new().capped_granary_purchase(&mut off, 0, &plan));
    assert!(!has_granary(&off, capital));

    let (mut on, capital) = capped_capital();
    let mut ai = AdvancedAi::new();
    ai.enable_gold_buys_the_capped_granary();
    let gold = on.players[0].gold;
    assert!(ai.capped_granary_purchase(&mut on, 0, &plan));
    assert!(has_granary(&on, capital), "the Granary stands");
    assert!(on.players[0].gold < gold, "it was paid for");
    assert!(
        !ai.capped_granary_purchase(&mut on, 0, &plan),
        "one Granary"
    );
}

/// A city with housing to spare, an empty treasury, or a threatened city in
/// the plan buys nothing.
#[test]
fn room_to_grow_no_gold_or_a_threat_buys_nothing() {
    let mut ai = AdvancedAi::new();
    ai.enable_gold_buys_the_capped_granary();

    let (mut roomy, capital) = capped_capital();
    roomy.cities.get_mut(&capital).unwrap().pop = 1;
    let plan = plan_for(&roomy);
    assert!(
        !ai.capped_granary_purchase(&mut roomy, 0, &plan),
        "room to grow"
    );

    let (mut poor, _) = capped_capital();
    poor.players[0].gold = 10.0;
    assert!(!ai.capped_granary_purchase(&mut poor, 0, &plan), "no gold");

    let (mut threatened, capital) = capped_capital();
    let mut alarm = plan_for(&threatened);
    alarm.threatened_city = Some(capital);
    assert!(
        !ai.capped_granary_purchase(&mut threatened, 0, &alarm),
        "a threat"
    );
}
