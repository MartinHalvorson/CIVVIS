use super::*;

/// A founder's capital with one Apostle of its faith on it.
fn board(charges: i32) -> (Game, u32) {
    let mut game = Game::new_full(2, 30, 18, 7_116, 200, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    game.players[0].religion = Some("Our Faith".to_string());
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let uid = game.spawn_unit("apostle", 0, home);
    let unit = game.units.get_mut(&uid).unwrap();
    unit.religion = Some("Our Faith".to_string());
    unit.charges = charges;
    (game, uid)
}

fn gene() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_apostle_evangelizes_unspent();
    ai
}

#[test]
fn the_ruleset_charge_count_is_what_the_fixture_expects() {
    let (game, _) = board(3);
    assert_eq!(
        game.rules.units[crate::name!("apostle")].charges,
        3,
        "the live 2-charge refusals are read against this count"
    );
}

#[test]
fn an_unspent_apostle_may_evangelize() {
    let (game, uid) = board(3);
    assert!(gene().apostle_may_evangelize(&game, uid));
}

/// A charge above the ruleset count comes from a belief or promotion.
#[test]
fn an_apostle_with_extra_charges_may_evangelize() {
    let (game, uid) = board(5);
    assert!(gene().apostle_may_evangelize(&game, uid));
}

/// Live: 82 of 82 two-charge asks were refused.
#[test]
fn an_apostle_that_spent_a_charge_may_not() {
    let (game, uid) = board(2);
    assert!(!gene().apostle_may_evangelize(&game, uid));
}

#[test]
fn off_nothing_changes() {
    let (game, uid) = board(2);
    assert!(AdvancedAi::new().apostle_may_evangelize(&game, uid));
}
