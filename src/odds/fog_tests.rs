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
    assert_eq!(no_city_penalty(&game, 1, true), 0.0);
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
    assert_eq!(no_city_penalty(&game, 1, true), 0.0);
    assert!(still_playing(&game, 1, true));
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

#[test]
fn host_rebuild_keeps_an_entirely_unlocated_empire_in_the_odds() {
    use crate::mirror::{rebuild_from_state, state_from_json, Snapshot, TilesChunk};
    let state = state_from_json(
        r#"{
        "turn":105,"score":300,
        "cities":[{"id":1,"name":"Home","x":6,"y":6,"pop":3}],
        "units":[],
        "rivals":[{"player":1,"score":600,"military":500,
                   "public_stats":{"city_count":6},"cities":[],"units":[]}]
    }"#,
    )
    .unwrap();
    let plots = (0..14)
        .flat_map(|y| {
            (0..20).map(move |x| {
                serde_json::from_value(serde_json::json!({
                    "x":x,"y":y,"t":"TERRAIN_GRASS","vis":true
                }))
                .unwrap()
            })
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 105,
        width: 20,
        height: 14,
        chunk: 1,
        plots,
    }]);
    let game = rebuild_from_state(&snapshot, &state, 2, 3626, 400, 0).game;
    assert!(game.player_city_ids(1).is_empty());
    assert!(game.player_unit_ids(1).is_empty());
    assert!(game.unseen_city_owners.contains(&1));
    assert_eq!(city_count(&game, 1), 6);
    let odds = table(&game, flat);
    assert!(
        odds[&1].now > 0.5,
        "the host's stronger rival remains favoured: {:?}",
        odds[&1]
    );
    assert_eq!(
        mercy_leader(&game, 0.99),
        None,
        "an empty fog roster must not produce a false mercy winner"
    );
}

#[test]
fn unmet_empires_keep_their_prior_without_fabricated_collapse_or_mercy() {
    let mut game = even_table(3, "prince", &[]);
    found_capital(&mut game, 0);
    for pid in 1..3 {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    Arc::make_mut(&mut game.observed_public_empire_stats).insert(
        0,
        ObservedPublicEmpireStats {
            city_count: Some(1),
            ..Default::default()
        },
    );
    game.turn = 100;
    let odds = table(&game, flat);
    for pid in 0..3 {
        assert!(
            (odds[&pid].now - 1.0 / 3.0).abs() < 1e-12,
            "no comparative information means the even prior survives: {:?}",
            odds
        );
    }
    assert_eq!(mercy_leader(&game, 0.9), None);
    game.players[1].alive = false;
    assert_eq!(table(&game, flat)[&1].now, 0.0);
}

#[test]
fn an_observed_zero_city_total_is_distinct_from_an_unobserved_empire() {
    let mut game = even_table(2, "prince", &[]);
    found_capital(&mut game, 0);
    for uid in game.player_unit_ids(1) {
        game.remove_unit(uid);
    }
    Arc::make_mut(&mut game.observed_public_empire_stats).insert(
        1,
        ObservedPublicEmpireStats {
            city_count: Some(0),
            ..Default::default()
        },
    );
    assert_eq!(table(&game, flat)[&1].now, 0.0);
}
