use super::*;

/// Our founded capital and a Settler of ours standing five tiles out; every
/// other unit removed.
fn capital_and_settler() -> (Game, u32) {
    let mut game = Game::new_full(2, 32, 20, 936_431, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    for pid in 0..2 {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let site = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| game.wdist(**pos, home) == 5 && !game.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .min()
        .expect("fixture: land five tiles out");
    game.turn = 90;
    let uid = game.spawn_unit("settler", 0, site);
    (game, uid)
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    if gene {
        ai.enable_settler_brake_counts_the_wanderer();
    }
    ai
}

/// A Settler out fifteen turns closes the pipeline from three cities; inside
/// the fifteen turns, below three cities, or with the gene off it does not.
#[test]
fn a_settler_wandering_fifteen_turns_closes_the_pipeline() {
    let (game, uid) = capital_and_settler();
    let mut ai = seat(true);
    ai.settler_walk_started
        .insert(uid, game.turn - WANDERER_TURNS);
    assert_eq!(ai.wandering_settlers(&game, 0), 1);
    assert!(ai.wanderer_closes_the_pipeline(&game, 0, 3));
    // Below the backlog brake's city floor the opening keeps expanding.
    assert!(!ai.wanderer_closes_the_pipeline(&game, 0, 2));

    // A Settler out fewer turns is still on its way.
    ai.settler_walk_started
        .insert(uid, game.turn - WANDERER_TURNS + 1);
    assert_eq!(ai.wandering_settlers(&game, 0), 0);
    assert!(!ai.wanderer_closes_the_pipeline(&game, 0, 3));

    // Off, nothing is counted or closed.
    let mut off = seat(false);
    off.settler_walk_started.insert(uid, 0);
    assert_eq!(off.wandering_settlers(&game, 0), 0);
    assert!(!off.wanderer_closes_the_pipeline(&game, 0, 5));
}

/// The empire clock holds a Settler the per-unit clock never stamped (live
/// Qin `civvis-t7-20261123T080638Z`): fifteen turns with a Settler out and no
/// city founded close the pipeline from three cities; founding a city, or no
/// Settler left, restarts it.
#[test]
fn the_empire_clock_holds_a_settler_no_unit_clock_saw() {
    let (mut game, _uid) = capital_and_settler();
    let mut ai = seat(true);
    // No per-unit clock at all.
    assert!(ai.settler_walk_started.is_empty());
    ai.note_settlers_out(&game, 0);
    assert!(!ai.wanderer_closes_the_pipeline(&game, 0, 3), "just out");
    game.turn += WANDERER_TURNS;
    ai.note_settlers_out(&game, 0);
    assert!(
        ai.wanderer_closes_the_pipeline(&game, 0, 3),
        "out fifteen turns, nothing founded"
    );
    assert!(
        !ai.wanderer_closes_the_pipeline(&game, 0, 2),
        "below the city floor"
    );

    // A city founded restarts the clock: the count it compares against grows.
    ai.settlers_out_since = Some((game.turn - WANDERER_TURNS, 0));
    ai.note_settlers_out(&game, 0);
    assert_eq!(ai.settlers_out_since, Some((game.turn, 1)));
    assert!(
        !ai.wanderer_closes_the_pipeline(&game, 0, 3),
        "a new city restarts the clock"
    );

    // No Settler left clears it.
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    ai.note_settlers_out(&game, 0);
    assert_eq!(ai.settlers_out_since, None);

    // Off, the clock is never kept.
    let mut off = seat(false);
    off.note_settlers_out(&game, 0);
    assert_eq!(off.settlers_out_since, None);
}
