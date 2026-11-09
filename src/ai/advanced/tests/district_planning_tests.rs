//! Tests of `advanced/district_planning.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

#[test]
fn coast_pairs_count_coast_and_lake_neighbours_in_twos() {
    let (mut game, _city, center) = planning_capital();
    let around = game.nbrs(center);
    for p in around.iter() {
        game.map.tiles.get_mut(p).unwrap().terrain = crate::name!("grassland");
    }
    assert_eq!(AdvancedAi::coast_pairs(&game, center), 0);
    game.map.tiles.get_mut(&around[0]).unwrap().terrain = crate::name!("coast");
    assert_eq!(
        AdvancedAi::coast_pairs(&game, center),
        0,
        "one tile is half a pair"
    );
    game.map.tiles.get_mut(&around[1]).unwrap().terrain = crate::name!("lake");
    game.map.tiles.get_mut(&around[2]).unwrap().terrain = crate::name!("coast");
    assert_eq!(AdvancedAi::coast_pairs(&game, center), 1);
    assert!(
        AdvancedAi::keeps_the_coast_credit(
            &game,
            0,
            crate::name!("commercial_hub"),
            center,
            around[3]
        ),
        "a district outside Great Nusantara always passes"
    );
}

#[test]
fn great_nusantara_guards_a_theater_square_like_the_other_three() {
    let (mut game, _city, center) = planning_capital();
    game.players[0].civ = "Indonesia".to_string();
    let around = game.nbrs(center);
    game.map.tiles.get_mut(&around[0]).unwrap().terrain = crate::name!("coast");
    game.map.tiles.get_mut(&around[1]).unwrap().terrain = crate::name!("coast");
    assert_eq!(AdvancedAi::coast_pairs(&game, center), 1);
    assert_eq!(AdvancedAi::coast_pairs(&game, around[3]), 0);
    for district in ["campus", "holy_site", "industrial_zone", "theater_square"] {
        assert!(
            !AdvancedAi::keeps_the_coast_credit(
                &game,
                0,
                crate::name::Name::new(district),
                center,
                around[3]
            ),
            "{district} must not trade a Coast pair away"
        );
    }
}
