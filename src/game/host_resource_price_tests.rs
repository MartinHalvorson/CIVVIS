use super::*;

fn bombard_seat() -> (Game, u32, Item) {
    let mut game = Game::new_full(2, 26, 16, 74_110, 250, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    game.found_city_for(0, game.units[&settler].pos, None);
    game.players[0].civ = "Egypt".to_string();
    game.players[0].techs.insert(crate::name!("metal_casting"));
    let city = game.player_city_ids(0)[0];
    let item = Item::Unit {
        unit: crate::name!("bombard"),
    };
    (game, city, item)
}

#[test]
fn native_unit_price_controls_eligibility_and_the_material_committed() {
    let (mut game, city, item) = bombard_seat();
    game.players[0]
        .strategic_resources
        .insert(crate::name!("niter"), 10.0);
    assert!(!game.can_produce(0, city, &item));
    game.replace_host_unit_resource_prices(
        [(city, [(Game::production_block_key(&item), 10.0)].into())].into(),
    );
    assert!(game.can_produce(0, city, &item));
    assert!(game.commit_unit_resource(0, city, &item));
    assert_eq!(game.strategic_stockpile(0, crate::name!("niter")), 0.0);
    assert!(game.unit_resource_is_committed(city, &item));
    assert!(
        game.commit_unit_resource(0, city, &item),
        "resuming the same order must not charge twice"
    );
    assert_eq!(game.strategic_stockpile(0, crate::name!("niter")), 0.0);
}

#[test]
fn native_formation_price_is_a_complete_bill_and_stays_city_specific() {
    let (mut game, city, item) = bombard_seat();
    let corps = Item::Formation {
        unit: crate::name!("bombard"),
        formation: 1,
    };
    let army = Item::Formation {
        unit: crate::name!("bombard"),
        formation: 2,
    };
    game.replace_host_unit_resource_prices(
        [(
            city,
            [
                (Game::production_block_key(&item), 10.0),
                (Game::production_block_key(&corps), 18.0),
                (Game::production_block_key(&army), 25.0),
            ]
            .into(),
        )]
        .into(),
    );
    assert_eq!(game.unit_resource_cost(city, &corps), 18.0);
    assert_eq!(game.unit_resource_cost(city, &army), 25.0);
    game.players[0]
        .strategic_resources
        .insert(crate::name!("niter"), 18.0);
    assert!(game.commit_unit_resource(0, city, &corps));
    assert_eq!(game.strategic_stockpile(0, crate::name!("niter")), 0.0);
    game.replace_host_unit_resource_prices(
        [(
            city + 100,
            [(Game::production_block_key(&item), 1.0)].into(),
        )]
        .into(),
    );
    assert_eq!(game.unit_resource_cost(city, &item), 20.0);
}

#[test]
fn zero_is_a_price_but_invalid_and_absent_readings_use_the_rules() {
    let (mut game, city, item) = bombard_seat();
    game.replace_host_unit_resource_prices(
        [(city, [(Game::production_block_key(&item), 0.0)].into())].into(),
    );
    assert!(game.can_produce(0, city, &item));
    assert!(game.commit_unit_resource(0, city, &item));
    assert_eq!(game.strategic_stockpile(0, crate::name!("niter")), 0.0);
    for invalid in [-1.0, f64::NAN, f64::INFINITY] {
        game.replace_host_unit_resource_prices(
            [(city, [(Game::production_block_key(&item), invalid)].into())].into(),
        );
        assert_eq!(game.unit_resource_cost(city, &item), 20.0);
        assert!(!game.can_produce(0, city, &item));
    }
    game.replace_host_unit_resource_prices(BTreeMap::new());
    assert_eq!(game.unit_resource_cost(city, &item), 20.0);
}
