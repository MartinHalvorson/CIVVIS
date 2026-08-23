use super::*;

#[test]
fn production_catalog_is_reused_until_a_successful_action_changes_the_world() {
    let mut game = Game::new_full(2, 24, 16, 91_171, 100, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("the opening roster includes a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found the opening city");
    let city = game.player_city_ids(0)[0];

    let first = game.producible_items(0, city);
    assert!(!first.is_empty(), "an opening city has a production menu");
    assert_eq!(game.query_memo.producible.borrow().len(), 1);
    assert_eq!(
        game.producible_items(0, city),
        first,
        "a second helper reads the cached catalog"
    );

    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    assert!(first.contains(&warrior));
    game.apply(0, &Action::Produce { city, item: warrior })
        .expect("a successful action invalidates the cached read state");
    assert!(
        game.query_memo.producible.borrow().is_empty(),
        "the next decision must derive its catalog from the new game state"
    );
}
