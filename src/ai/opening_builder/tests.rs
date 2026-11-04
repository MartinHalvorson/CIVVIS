use super::*;

/// A one-city opening: the capital founded on open grassland, its queue
/// empty, a Warrior at home and nothing else on the board.
fn opening() -> (Game, BasicAi, u32) {
    let mut g = Game::new_full(2, 24, 16, 91_780, 250, 0, false);
    g.current = 0;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = g.player_city_ids(0)[0];
    let home = g.cities[&capital].pos;
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.spawn_test_unit("warrior", 0, home);
    let mut ai = BasicAi::new();
    ai.opening_builder_after_the_first_settler = true;
    (g, ai, capital)
}

fn builder() -> Item {
    Item::Unit {
        unit: crate::name!("builder"),
    }
}

#[test]
fn the_capital_trains_a_builder_once_its_first_settler_walks() {
    let (mut g, mut ai, capital) = opening();
    // No Settler out yet: the opening is not this gene's to take.
    assert!(!ai.play_opening_builder(&mut g, 0, capital, 1, 0, 0));
    assert!(g.cities[&capital].queue.is_empty());

    assert!(ai.play_opening_builder(&mut g, 0, capital, 1, 1, 0));
    assert_eq!(g.cities[&capital].queue, vec![builder()]);
    assert!(ai.opening_builder_paid);

    // Once a game: after the receipt the step never fires again.
    g.cities.get_mut(&capital).unwrap().queue.clear();
    assert!(!ai.play_opening_builder(&mut g, 0, capital, 2, 0, 0));
}

#[test]
fn an_empire_that_already_holds_a_builder_is_left_alone() {
    let (mut g, mut ai, capital) = opening();
    assert!(!ai.play_opening_builder(&mut g, 0, capital, 2, 1, 1));
    assert!(g.cities[&capital].queue.is_empty());
}

#[test]
fn the_step_is_off_without_the_gene() {
    let (mut g, mut ai, capital) = opening();
    ai.opening_builder_after_the_first_settler = false;
    assert!(!ai.play_opening_builder(&mut g, 0, capital, 2, 1, 0));
}

#[test]
fn a_major_war_keeps_the_queue_for_the_ordinary_picker() {
    let (mut g, mut ai, capital) = opening();
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    assert!(!ai.play_opening_builder(&mut g, 0, capital, 2, 1, 0));
    assert!(g.cities[&capital].queue.is_empty());
}

#[test]
fn the_controller_queues_the_builder_ahead_of_the_next_settler() {
    let (mut g, mut ai, capital) = opening();
    g.cities.get_mut(&capital).unwrap().pop = 2;
    ai.capital_settler_after_completion = true;
    let home = g.cities[&capital].pos;
    g.spawn_test_unit("settler", 0, home);
    ai.cities(&mut g, 0);
    assert_eq!(g.cities[&capital].queue[0], builder());
}
