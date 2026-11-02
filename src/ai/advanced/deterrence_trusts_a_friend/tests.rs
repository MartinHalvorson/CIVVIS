use super::*;
use crate::ai::advanced::near_rival_deterrence::{
    DETERRENCE_NEAR_TILES, DETERRENCE_START_STANDARD,
};
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// Two met majors with a city each inside the neighbour radius, inside the
/// deterrence window, the rival out-gunning us two to one.
fn neighbours() -> Game {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (5..=DETERRENCE_NEAR_TILES).contains(&game.wdist(**pos, home))
                && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(1, sites[0], None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    std::sync::Arc::make_mut(&mut game.observed_military_power).insert(0, 100.0);
    std::sync::Arc::make_mut(&mut game.observed_military_power).insert(1, 200.0);
    game.turn = game.standard_duration(DETERRENCE_START_STANDARD) + 5;
    game
}

fn deterring(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_near_rival_deterrence();
    if gene {
        ai.enable_deterrence_trusts_a_friend();
    }
    ai
}

#[test]
fn deterrence_trusts_a_friend_is_opt_in() {
    opt_in_off_in_both_controllers("deterrence-trusts-a-friend", |ai| {
        ai.deterrence_trusts_a_friend
    });
}

/// A declared friend is not a deterrence neighbour under the gene; off, it is.
#[test]
fn a_declared_friend_is_not_deterred() {
    let mut game = neighbours();
    assert!(deterring(false).deterrence_threat(&game, 0).is_some());
    assert!(
        deterring(true).deterrence_threat(&game, 0).is_some(),
        "not friends yet"
    );
    let until = game.turn + 30;
    game.players[0].friends_until.insert(1, until);
    game.players[1].friends_until.insert(0, until);
    assert!(game.are_friends(0, 1));
    assert!(
        deterring(false).deterrence_threat(&game, 0).is_some(),
        "stock deters a friend"
    );
    assert!(
        deterring(true).deterrence_threat(&game, 0).is_none(),
        "the gene trusts the friend"
    );
}

/// When the friendship lapses the deterrent answers again.
#[test]
fn a_lapsed_friendship_deters_again() {
    let mut game = neighbours();
    let until = game.turn;
    game.players[0].friends_until.insert(1, until);
    game.players[1].friends_until.insert(0, until);
    assert!(!game.are_friends(0, 1), "the friendship has lapsed");
    assert!(deterring(true).deterrence_threat(&game, 0).is_some());
}
