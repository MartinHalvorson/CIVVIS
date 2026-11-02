use super::*;

/// One major, our capital founded at turn 20 at population 3 with every unit
/// removed, a bank of `gold` and a zero income. Returns the game and the
/// capital.
fn young_empire(gold: f64) -> Option<(Game, u32)> {
    let mut game = Game::new_full(1, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")?;
    game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let capital = game.player_city_ids(0)[0];
    game.turn = 20;
    game.cities.get_mut(&capital).unwrap().pop = 3;
    game.players[0].gold = gold;
    game.players[0].gold_per_turn = 0.0;
    Some((game, capital))
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_gold_buys_the_opening_builder();
    ai
}

fn builders(game: &Game) -> usize {
    game.player_unit_ids(0)
        .into_iter()
        .filter(|uid| game.units[uid].kind == "builder")
        .count()
}

/// A bank that covers the Builder above the floor buys it with the gene on
/// and is untouched with it off.
#[test]
fn a_builderless_opening_buys_its_builder() {
    let Some((game, capital)) = young_empire(2_000.0) else {
        panic!("fixture: a young empire");
    };
    assert!(
        game.unit_purchase_cost(0, capital, "builder", "gold")
            .is_some(),
        "fixture: the capital can buy a Builder"
    );

    let stock = AdvancedAi::new();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    assert!(!stock.opening_builder_purchase(&mut off, 0, &plan));
    assert_eq!(builders(&off), 0);
    assert_eq!(off.players[0].gold, game.players[0].gold);

    let ai = armed();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    assert!(ai.opening_builder_purchase(&mut on, 0, &plan));
    assert_eq!(builders(&on), 1, "on: one Builder bought");
    assert!(on.players[0].gold < game.players[0].gold, "on: Gold spent");

    // One Builder afield closes the window: no second purchase.
    let plan = ai.assess(&on, 0);
    assert!(!ai.opening_builder_purchase(&mut on, 0, &plan));
    assert_eq!(builders(&on), 1);
}

/// An empty bank buys nothing, and the window shuts at the standard turn
/// limit.
#[test]
fn the_purchase_needs_the_gold_and_the_opening() {
    let ai = armed();
    let Some((mut poor, _)) = young_empire(0.0) else {
        panic!("fixture: a young empire");
    };
    let plan = ai.assess(&poor, 0);
    assert!(!ai.opening_builder_purchase(&mut poor, 0, &plan));
    assert_eq!(builders(&poor), 0);

    let Some((mut late, _)) = young_empire(2_000.0) else {
        panic!("fixture: a young empire");
    };
    late.turn = late.standard_duration(OPENING_BUILDER_END_STANDARD);
    let plan = ai.assess(&late, 0);
    assert!(!ai.opening_builder_purchase_open(&late, 0, &plan));
    assert!(!ai.opening_builder_purchase(&mut late, 0, &plan));
    assert_eq!(builders(&late), 0);
}

#[test]
fn gold_buys_the_opening_builder_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "gold-buys-the-opening-builder",
        |ai| ai.gold_buys_the_opening_builder,
    );
}
