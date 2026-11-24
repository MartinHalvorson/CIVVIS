use super::*;

fn walked_game() -> Game {
    let mut game = Game::new_full(1, 24, 16, 5_761, 300, 0, false);
    game.chronicle.unit_move_trails_mut().push(UnitMoveTrail {
        unit: 1,
        owner: 0,
        turn: game.turn,
        path: vec![(3, 3), (4, 3)],
    });
    game.chronicle.turn_stats_mut().push(TurnStats {
        turn: game.turn,
        player: 0,
        cities: 1,
        science: 2.0,
        culture: 1.0,
        production: 3.0,
        faith: 0.0,
        gold: 5.0,
    });
    game
}

/// The rules never read the chronicle back (`docs/CORE_STATE.md`), so a fork
/// of the world must not pay to copy it: every record is a shared pointer
/// until the fork writes to that record.
#[test]
fn a_fork_shares_the_chronicle_until_it_writes() {
    let game = walked_game();
    let mut fork = game.clone();
    let (a, b) = (&game.chronicle, &fork.chronicle);
    assert!(Arc::ptr_eq(&a.concluded_wars, &b.concluded_wars));
    assert!(Arc::ptr_eq(&a.nuclear_strikes, &b.nuclear_strikes));
    assert!(Arc::ptr_eq(&a.turn_stats, &b.turn_stats));
    assert!(Arc::ptr_eq(&a.unit_move_trails, &b.unit_move_trails));

    fork.chronicle.unit_move_trails_mut()[0].path.push((5, 3));
    assert!(!Arc::ptr_eq(
        &game.chronicle.unit_move_trails,
        &fork.chronicle.unit_move_trails
    ));
    assert_eq!(game.chronicle.unit_move_trails[0].path.len(), 2);
    assert_eq!(fork.chronicle.unit_move_trails[0].path.len(), 3);
    // Writing one record leaves the others shared.
    assert!(Arc::ptr_eq(
        &game.chronicle.turn_stats,
        &fork.chronicle.turn_stats
    ));
}

/// A save still carries the chronicle under its old top-level names, so
/// saves written before and after it was grouped read the same.
#[test]
fn the_chronicle_round_trips_through_a_save() {
    let game = walked_game();
    let text = serde_json::to_string(&game).unwrap();
    let save: serde_json::Value = serde_json::from_str(&text).unwrap();
    for key in [
        "concluded_wars",
        "nuclear_strikes",
        "turn_stats",
        "unit_move_trails",
    ] {
        assert!(save.get(key).is_some(), "the save names {key} at the top");
    }
    assert!(save.get("chronicle").is_none());
    let back: Game = serde_json::from_str(&text).unwrap();
    assert_eq!(
        back.chronicle.unit_move_trails,
        game.chronicle.unit_move_trails
    );
    assert_eq!(back.chronicle.turn_stats, game.chronicle.turn_stats);
}
