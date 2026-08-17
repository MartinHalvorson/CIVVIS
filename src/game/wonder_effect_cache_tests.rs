use super::{Action, Game};

#[test]
fn per_player_wonder_effect_aggregate_matches_the_scalar_path() {
    let mut game = Game::new_full(1, 20, 14, 91_483, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("the fixture has a starting Settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("the starting Settler can found the capital");
    let city = game.player_city_ids(0)[0];
    let position = game.cities[&city].pos;
    game.cities.get_mut(&city).unwrap().wonders.extend([
        (crate::name!("angkor_wat"), position),
        (crate::name!("alhambra"), position),
        (crate::name!("colossus"), position),
        (crate::name!("forbidden_city"), position),
        (crate::name!("kilwa_kisiwani"), position),
        (crate::name!("taj_mahal"), position),
    ]);
    let effects = [
        "empire_housing",
        "historic_moment_bonus",
        "military_policy_slots",
        "suzerain_type_empire_bonus_pct",
        "trade_route_capacity",
    ];
    let scalar: Vec<f64> = effects
        .iter()
        .map(|effect| game.empire_wonder_effect(0, effect))
        .collect();

    {
        let _memo = game.query_memo();
        let cached: Vec<f64> = effects
            .iter()
            .map(|effect| game.empire_wonder_effect(0, effect))
            .collect();
        assert_eq!(cached, scalar);
        assert_eq!(
            game.query_memo
                .wonder_effects
                .borrow()
                .as_ref()
                .expect("the guard opens the wonder aggregate")
                .len(),
            1
        );
    }
    assert!(game.query_memo.wonder_effects.borrow().is_none());

    // A later world state gets a fresh aggregate rather than retaining a
    // city's old wonder contribution across the memo boundary.
    game.cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .remove(&crate::name!("taj_mahal"));
    let without_taj = game.empire_wonder_effect(0, "historic_moment_bonus");
    assert_eq!(without_taj, 0.0);
    let with_new_memo = {
        let _memo = game.query_memo();
        game.empire_wonder_effect(0, "historic_moment_bonus")
    };
    assert_eq!(with_new_memo, without_taj);
}
