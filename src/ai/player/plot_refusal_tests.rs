use super::*;

fn board() -> (Game, u32, u32, u32) {
    let mut game = Game::new_full(2, 28, 18, 41, 30, 0, false);
    for id in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(id);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.owner_city = None;
    }
    let city = game.found_city_for(0, (8, 6), None);
    game.at_war.insert((0, 1));
    let target = game.spawn_unit("warrior", 1, (8, 8));
    game.units.get_mut(&target).unwrap().hp = 1000;
    let gun = game.spawn_unit("archer", 0, (8, 7));
    game.players[0].gold = 1000.0;
    (game, city, target, gun)
}

fn independent_tail(game: &Game, city: u32, gun: u32, target: u32) -> [(usize, Action); 2] {
    [
        (
            0,
            Action::Produce {
                city,
                item: Item::Unit {
                    unit: crate::name!("scout"),
                },
            },
        ),
        (
            0,
            Action::Ranged {
                unit: gun,
                target: game.units[&target].pos,
            },
        ),
    ]
}

fn assert_tail_executed(game: &Game, city: u32, target: u32) {
    assert!(
        !game.cities[&city].queue.is_empty(),
        "plot refusal starved production"
    );
    assert!(
        game.units[&target].hp < 1000,
        "plot refusal starved the army"
    );
    assert_eq!(game.players[0].counters["player:refused"], 1);
}

#[test]
fn a_border_whose_city_is_hidden_cannot_starve_independent_orders() {
    let (mut game, city, target, gun) = board();
    let foreign = game.found_city_for(1, (15, 6), None);
    let pos = (10, 6);
    game.map.tiles.get_mut(&pos).unwrap().owner_city = Some(foreign);
    game.cities.get_mut(&foreign).unwrap().owned_tiles.push(pos);
    let view = game.player_decision_view(0);
    assert!(
        !view.cities.contains_key(&foreign),
        "the foreign city is unseen"
    );
    assert!(
        view.unseen_major_borders.contains(&pos),
        "the border is visible"
    );
    let cost = view
        .plot_purchase_cost(0, city, pos)
        .expect("the projection offers the plot");
    assert!(game.plot_purchase_cost(0, city, pos).is_none());
    let tail = independent_tail(&game, city, gun, target);
    let gold = game.players[0].gold;
    assert!(execute_frame(
        &mut game,
        0,
        &Default::default(),
        [(0, Action::BuyPlot { city, pos, cost })]
            .iter()
            .chain(tail.iter())
    ));
    assert_tail_executed(&game, city, target);
    assert_eq!(game.players[0].gold, gold);
    assert_eq!(game.map.tiles[&pos].owner_city, Some(foreign));
}

#[test]
fn an_unaffordable_plot_cannot_starve_independent_orders() {
    let (mut game, city, target, gun) = board();
    let pos = (10, 6);
    assert!(game.plot_purchase_cost(0, city, pos).is_some());
    game.players[0].gold = 0.0;
    let rejected = Action::BuyPlot {
        city,
        pos,
        cost: 0.0,
    };
    assert_eq!(
        game.clone().apply(0, &rejected).unwrap_err(),
        "cannot afford"
    );
    let tail = independent_tail(&game, city, gun, target);
    assert!(execute_frame(
        &mut game,
        0,
        &Default::default(),
        [(0, rejected)].iter().chain(tail.iter())
    ));
    assert_tail_executed(&game, city, target);
    assert_eq!(game.players[0].gold, 0.0);
    assert_eq!(game.map.tiles[&pos].owner_city, None);
}

#[test]
fn continuation_still_pays_each_authoritative_plot_price() {
    let (mut game, city, target, gun) = board();
    let first = (10, 6);
    let second = (8, 4);
    let first_cost = game.plot_purchase_cost(0, city, first).unwrap();
    let second_cost = game.plot_purchase_cost(0, city, second).unwrap();
    game.players[0].gold = first_cost + second_cost - 1.0;
    let actions = [first, second].map(|pos| {
        (
            0,
            Action::BuyPlot {
                city,
                pos,
                cost: 0.0,
            },
        )
    });
    let tail = independent_tail(&game, city, gun, target);
    assert!(execute_frame(
        &mut game,
        0,
        &Default::default(),
        actions.iter().chain(tail.iter())
    ));
    assert_tail_executed(&game, city, target);
    assert_eq!(game.players[0].gold, second_cost - 1.0);
    assert_eq!(game.map.tiles[&first].owner_city, Some(city));
    assert_eq!(game.map.tiles[&second].owner_city, None);
}
