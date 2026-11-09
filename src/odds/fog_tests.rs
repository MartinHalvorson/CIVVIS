use super::*;
use crate::game::ObservedPublicEmpireStats;
use std::sync::Arc;

#[test]
fn hidden_cities_keep_a_living_rival_in_the_forecast() {
    let mut game = even_table(2, "prince", &[]);
    found_capital(&mut game, 0);
    // The live host knows the rival owns six cities without disclosing a
    // single location, settler, or unit. This is the mirror's real contract.
    for uid in game.player_unit_ids(1) {
        game.remove_unit(uid);
    }
    Arc::make_mut(&mut game.observed_public_empire_stats).insert(
        1,
        ObservedPublicEmpireStats {
            city_count: Some(6),
            ..Default::default()
        },
    );
    game.turn = 150;
    let odds = table(&game, flat);
    assert!(game.player_city_ids(1).is_empty());
    assert!(
        odds[&1].now > 0.5,
        "six cities cannot disappear behind fog: {:?}",
        odds[&1]
    );
    assert_eq!(no_city_penalty(&game, 1), 0.0);
    assert!((odds[&0].now + odds[&1].now - 1.0).abs() < 1e-12);

    // A confirmed elimination still outranks an old public count.
    game.players[1].alive = false;
    assert_eq!(table(&game, flat)[&1].now, 0.0);
}

#[test]
fn public_city_totals_value_the_empire_instead_of_the_revealed_map() {
    let mut game = even_table(2, "prince", &[]);
    for pid in 0..2 {
        found_capital(&mut game, pid);
    }
    game.turn = 150;
    let before = table(&game, flat);
    Arc::make_mut(&mut game.observed_public_empire_stats).insert(
        1,
        ObservedPublicEmpireStats {
            city_count: Some(8),
            ..Default::default()
        },
    );
    game.unseen_city_owners.insert(1);
    let after = table(&game, flat);
    assert!(after[&1].now > before[&1].now + 0.1);
    assert_eq!(before[&1].start, after[&1].start);
    assert_eq!(city_count(&game, 1), 8);
}

#[test]
fn hidden_city_existence_does_not_require_an_exact_count() {
    let mut game = even_table(2, "prince", &[]);
    game.unseen_city_owners.insert(1);
    game.turn = 150;
    assert_eq!(city_count(&game, 1), 1);
    assert_eq!(no_city_penalty(&game, 1), 0.0);
    assert!(still_playing(&game, 1));
}

#[test]
fn stale_public_zero_cannot_erase_a_city_on_the_board() {
    let mut game = even_table(2, "prince", &[]);
    found_capital(&mut game, 1);
    Arc::make_mut(&mut game.observed_public_empire_stats).insert(
        1,
        ObservedPublicEmpireStats {
            city_count: Some(0),
            ..Default::default()
        },
    );
    assert_eq!(city_count(&game, 1), 1);
}
