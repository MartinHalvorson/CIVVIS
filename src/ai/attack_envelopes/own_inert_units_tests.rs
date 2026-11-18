use super::*;
use crate::game::{Action, Game};

/// A grassland board with one seat-1 warrior at war with seat 0, and a
/// seat-0 Builder standing inside that warrior's movement field.
fn board() -> (Game, u32, u32, u32) {
    let mut g = Game::new_full(2, 24, 16, 5_150, 300, 0, false);
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
    assert!(g.is_at_war(0, 1));
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    let centre = positions[positions.len() / 2];
    let enemy = g.spawn_test_unit("warrior", 1, centre);
    let near = g.nbrs(centre).into_iter().next().unwrap();
    let builder = g.spawn_test_unit("builder", 0, g.nbrs(near).into_iter().next_back().unwrap());
    let soldier = g.spawn_test_unit("warrior", 0, positions[3]);
    (g, enemy, builder, soldier)
}

fn step(g: &mut Game, uid: u32) {
    let from = g.units[&uid].pos;
    let to = g
        .nbrs(from)
        .into_iter()
        .find(|pos| g.unit_ids_at(*pos).is_empty())
        .unwrap();
    g.units.get_mut(&uid).unwrap().pos = to;
}

/// The reason the asker's own Builder may be left out: the at-war enemy's
/// attack reach and strike reach read the same before and after it walks,
/// over every tile of the enemy's field.
#[test]
fn an_own_builder_walking_changes_no_enemy_reach() {
    let (mut g, enemy, builder, _) = board();
    for _ in 0..6 {
        let attack = g.attack_reach(enemy);
        let strike = crate::ai::advanced::movement_strike_reach(&g, 0, enemy);
        step(&mut g, builder);
        assert_eq!(g.attack_reach(enemy), attack);
        assert_eq!(
            crate::ai::advanced::movement_strike_reach(&g, 0, enemy),
            strike
        );
    }
}

#[test]
fn the_askers_own_builder_is_left_out_and_a_soldier_is_not() {
    let (mut g, _, builder, soldier) = board();
    let ai = BasicAi::new();
    assert!(
        ai.envelope_board_change(&g, Some(0)).touched.is_none(),
        "first ask"
    );
    step(&mut g, builder);
    assert_eq!(
        ai.envelope_board_change(&g, Some(0)).touched,
        Some(Vec::new()),
        "seat 0's own Builder cannot touch an envelope told to seat 0"
    );
    step(&mut g, soldier);
    assert!(
        ai.envelope_board_change(&g, Some(0))
            .touched
            .is_some_and(|touched| !touched.is_empty()),
        "a soldier exerts zone of control and is reported"
    );
}

#[test]
fn another_seat_asking_after_a_skip_starts_over() {
    let (mut g, _, builder, _) = board();
    let ai = BasicAi::new();
    assert!(ai.envelope_board_change(&g, Some(0)).touched.is_none());
    step(&mut g, builder);
    assert_eq!(
        ai.envelope_board_change(&g, Some(0)).touched,
        Some(Vec::new())
    );
    assert!(
        ai.envelope_board_change(&g, Some(1)).touched.is_none(),
        "seat 0's Builder was left out, and seat 1 may be at peace with it"
    );
    step(&mut g, builder);
    assert!(
        ai.envelope_board_change(&g, Some(1))
            .touched
            .is_some_and(|touched| !touched.is_empty()),
        "to seat 1, seat 0's Builder is reported as before"
    );
}

#[test]
fn without_an_asker_every_change_is_reported() {
    let (mut g, _, builder, _) = board();
    let ai = BasicAi::new();
    assert!(ai.envelope_board_change(&g, None).touched.is_none());
    step(&mut g, builder);
    assert!(ai
        .envelope_board_change(&g, None)
        .touched
        .is_some_and(|touched| !touched.is_empty()));
}

/// A tile written far outside an enemy's movement field and attack range
/// leaves its envelope standing; one written inside it does not. Checked
/// against a fresh computation both ways.
#[test]
fn a_far_tile_write_keeps_an_envelope_and_a_near_one_does_not() {
    let (mut g, enemy, _, _) = board();
    let ai = BasicAi::new();
    let first = ai.enemy_attack_envelopes(&g, 0);
    let reach = |envelopes: &AttackEnvelopes| {
        envelopes
            .iter()
            .find(|(id, _)| *id == enemy)
            .map(|(_, reach)| std::sync::Arc::clone(reach))
            .expect("the enemy has an envelope")
    };
    let centre = g.units[&enemy].pos;
    let far = *g
        .map
        .tiles
        .keys()
        .max_by_key(|pos| (g.wdist(**pos, centre), **pos))
        .unwrap();
    assert!(g.wdist(far, centre) > 6, "the board has a far tile");
    g.map.tiles.get_mut(&far).unwrap().terrain = crate::name!("mountain");
    let kept = ai.enemy_attack_envelopes(&g, 0);
    assert!(
        std::sync::Arc::ptr_eq(&reach(&first), &reach(&kept)),
        "a mountain raised out of reach does not re-flood the enemy"
    );
    assert_eq!(
        *reach(&kept),
        EnvelopeReach::from_tiles(g.attack_reach(enemy))
    );
    let near = g.nbrs(centre).into_iter().next().unwrap();
    g.map.tiles.get_mut(&near).unwrap().terrain = crate::name!("mountain");
    let redone = ai.enemy_attack_envelopes(&g, 0);
    assert!(
        !std::sync::Arc::ptr_eq(&reach(&kept), &reach(&redone)),
        "a mountain raised beside it does"
    );
    assert_eq!(
        *reach(&redone),
        EnvelopeReach::from_tiles(g.attack_reach(enemy))
    );
}
