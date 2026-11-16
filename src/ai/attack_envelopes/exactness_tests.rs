use super::*;
use crate::game::{Action, Game};

/// Seat 0 at war with seat 1, a seat-1 warrior in sight of a seat-0 warrior.
fn board() -> (Game, u32, u32) {
    let mut g = Game::new_full(2, 24, 16, 5_152, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.cities.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.owner_city = None;
    }
    g.record_contact(0, 1);
    g.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    let centre = positions[positions.len() / 2];
    let enemy = g.spawn_test_unit("warrior", 1, centre);
    let near = g.nbrs(centre).into_iter().nth(2).unwrap();
    let ours = g.spawn_test_unit("warrior", 0, g.nbrs(near).into_iter().nth(2).unwrap());
    (g, enemy, ours)
}

/// The outer table is handed back whole on a key hit, so its key moves with
/// every input the per-enemy delta and stamp track.
#[test]
fn the_outer_key_moves_with_the_lock_the_link_the_patrol_the_seat_and_the_wars() {
    let (g, enemy, ours) = board();
    let key = |g: &Game| BasicAi::attack_envelope_fingerprint(g, None);
    let before = key(&g);
    type Edit = (&'static str, fn(&mut Game, u32, u32));
    let edits: [Edit; 7] = [
        ("started_turn_in_zoc", |g, e, _| {
            let u = g.units.get_mut(&e).unwrap();
            u.started_turn_in_zoc = !u.started_turn_in_zoc;
        }),
        ("acted", |g, e, _| {
            let u = g.units.get_mut(&e).unwrap();
            u.acted = !u.acted;
        }),
        ("moved", |g, e, _| {
            let u = g.units.get_mut(&e).unwrap();
            u.moved = !u.moved;
        }),
        ("linked_to", |g, e, o| {
            g.units.get_mut(&e).unwrap().linked_to = Some(o)
        }),
        ("air_patrol_pos", |g, e, _| {
            let u = g.units.get_mut(&e).unwrap();
            u.air_patrol = true;
            u.air_patrol_pos = Some((0, 0));
        }),
        ("current", |g, _, _| g.current = 1 - g.current),
        ("at_war", |g, _, _| {
            g.at_war.clear();
        }),
    ];
    for (field, edit) in edits {
        let mut board = g.clone();
        edit(&mut board, enemy, ours);
        assert_ne!(key(&board), before, "{field} must move the outer key");
    }
}

/// The per-enemy board lives one seat-turn: an enemy's reach reads its
/// owner's own state, which changes on its owner's turn and nowhere else.
#[test]
fn a_new_seat_turn_starts_the_store_over() {
    let (mut g, _, _) = board();
    let ai = BasicAi::new();
    assert!(
        ai.envelope_board_change(&g, Some(0)).touched.is_none(),
        "first ask"
    );
    assert_eq!(
        ai.envelope_board_change(&g, Some(0)).touched,
        Some(Vec::new())
    );
    g.current = 1 - g.current;
    assert!(
        ai.envelope_board_change(&g, Some(0)).touched.is_none(),
        "another seat's turn"
    );
}

/// An entry an ask did not read is dropped by any change that could have
/// reached it, so it cannot come back stale when its enemy is asked about
/// again.
#[test]
fn an_unread_entry_does_not_survive_a_change_that_reaches_it() {
    let (mut g, enemy, _) = board();
    let ai = BasicAi::new();
    let first = ai.enemy_attack_envelopes(&g, 0);
    assert!(first.iter().any(|(id, _)| *id == enemy));
    assert!(ai.enemy_envelope_cache.lock().unwrap().contains_key(&enemy));

    // Peace leaves the enemy out of the table (it is not read) and is a
    // whole-board change, so its entry must go.
    let at_war = g.at_war.clone();
    g.at_war.clear();
    let _ = ai.enemy_attack_envelopes(&g, 0);
    assert!(
        !ai.enemy_envelope_cache.lock().unwrap().contains_key(&enemy),
        "an entry nobody read through a whole-board change is dropped"
    );

    // Back at war, the enemy's reach is derived afresh and is what the board says.
    g.at_war = at_war;
    let again = ai.enemy_attack_envelopes(&g, 0);
    let reach = again
        .iter()
        .find(|(id, _)| *id == enemy)
        .map(|(_, reach)| std::sync::Arc::clone(reach))
        .expect("the enemy is back in the table");
    assert_eq!(*reach, EnvelopeReach::from_tiles(g.attack_reach(enemy)));
}
