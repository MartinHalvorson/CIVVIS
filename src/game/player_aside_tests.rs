use super::*;

/// `PlayerAside` is flattened into `Player`, and serde reads a flattened
/// struct from a buffered copy where every object key is a string; the
/// number-keyed maps must still come back.
#[test]
fn a_seats_aside_round_trips_through_a_save_under_its_old_names() {
    let mut game = Game::new_full(2, 24, 16, 5_764, 300, 0, false);
    game.players[0].observed_visibility.insert(1, 2.0);
    game.players[0]
        .observed_diplomatic_state
        .insert(1, "friendly".into());
    let save = serde_json::to_value(&game).unwrap();
    assert!(save["players"][0].get("observed_visibility").is_some());
    assert!(save["players"][0].get("aside").is_none());
    let back: Game = serde_json::from_value(save).unwrap();
    assert_eq!(back.players[0].observed_visibility.get(&1), Some(&2.0));
    assert_eq!(
        back.players[0]
            .observed_diplomatic_state
            .get(&1)
            .map(String::as_str),
        Some("friendly")
    );
}

/// A fork pays one pointer for a seat's aside, and copies it only when it
/// writes there.
#[test]
fn a_fork_shares_a_seats_aside_until_it_writes_one() {
    let game = Game::new_full(2, 24, 16, 5_764, 300, 0, false);
    let mut fork = game.clone();
    assert!(Arc::ptr_eq(
        &game.players[0].aside.0,
        &fork.players[0].aside.0
    ));
    fork.players[0].observed_visibility.insert(1, 1.0);
    assert!(!Arc::ptr_eq(
        &game.players[0].aside.0,
        &fork.players[0].aside.0
    ));
    assert!(game.players[0].observed_visibility.is_empty());
}
