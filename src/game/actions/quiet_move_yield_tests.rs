//! Rival yield reuse after a step, and the side effects that must reject it.
use super::*;

fn fixture() -> (Game, u32, u32, u32, Pos) {
    let mut game = Game::new_full(2, 24, 16, 91_171, 100, 0, false);
    for pid in 0..2 {
        while game.current != pid {
            let current = game.current;
            game.apply(current, &Action::EndTurn).unwrap();
        }
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| game.units[id].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    while game.current != 0 {
        let current = game.current;
        game.apply(current, &Action::EndTurn).unwrap();
    }
    let ground: BTreeSet<Pos> = game.map.tiles.keys().copied().collect();
    let seats = game.players.len();
    for pid in 0..seats {
        game.players[pid].explored = ground.clone();
        game.players[pid].met = (0..seats).filter(|other| *other != pid).collect();
    }
    let ours = game.player_city_ids(0)[0];
    let theirs = game.player_city_ids(1)[0];
    let (unit, to) = game
        .player_unit_ids(0)
        .into_iter()
        .filter(|id| game.rules.units[game.units[id].kind].class == "military")
        .find_map(|unit| {
            game.nbrs(game.units[&unit].pos).into_iter().find_map(|to| {
                (game.unit_ids_at(to).is_empty()
                    && game.city_at(to).is_none()
                    && game.map.tiles[&to].district.is_none()
                    && game.can_move(unit, to))
                .then_some((unit, to))
            })
        })
        .expect("an opening military unit has an empty adjacent move");
    game.keep_rival_yields(true);
    game.keep_own_yields(true);
    game.audit_rival_yields(true);
    (game, ours, theirs, unit, to)
}

fn bits(y: Yields) -> [u64; 6] {
    [y.food, y.production, y.gold, y.science, y.culture, y.faith].map(f64::to_bits)
}

#[test]
fn ordinary_steps_reuse_rivals_but_derive_our_own_again() {
    let (mut game, ours, theirs, unit, to) = fixture();
    game.city_yields(theirs);
    game.city_yields(ours);
    let before = game.rival_yields_reused().0;
    let mut fresh = game.clone();
    fresh.keep_rival_yields(false);
    let action = Action::Move { unit, to };
    game.apply(0, &action).unwrap();
    fresh.apply(0, &action).unwrap();
    assert_eq!(
        bits(game.city_yields(theirs)),
        bits(fresh.city_yields(theirs))
    );
    assert_eq!(game.rival_yields_reused().0, before + 1);
    let reused = game.rival_yields_reused().0;
    assert_eq!(bits(game.city_yields(ours)), bits(fresh.city_yields(ours)));
    assert_eq!(game.rival_yields_reused().0, reused, "our city was dropped");
}

#[test]
fn newly_explored_ground_drops_rivals() {
    let (mut game, _, theirs, unit, to) = fixture();
    game.players[0].explored.clear();
    game.city_yields(theirs);
    let before = game.rival_yields_reused().0;
    let mut fresh = game.clone();
    fresh.keep_rival_yields(false);
    let action = Action::Move { unit, to };
    game.apply(0, &action).unwrap();
    fresh.apply(0, &action).unwrap();
    assert!(!game.players[0].explored.is_empty());
    assert_eq!(
        bits(game.city_yields(theirs)),
        bits(fresh.city_yields(theirs))
    );
    assert_eq!(game.rival_yields_reused().0, before);
}

#[test]
fn contact_made_by_the_move_drops_rivals() {
    let (mut game, _, theirs, unit, to) = fixture();
    let near = game
        .nbrs(to)
        .into_iter()
        .find(|pos| {
            game.unit_ids_at(*pos).is_empty()
                && *pos != to
                && game.rules.is_passable(&game.map.tiles[pos])
                && !game.rules.is_water(&game.map.tiles[pos])
        })
        .unwrap();
    game.spawn_unit("warrior", 1, near);
    game.players[0].met.remove(&1);
    game.players[1].met.remove(&0);
    game.city_yields(theirs);
    let before = game.rival_yields_reused().0;
    game.apply(0, &Action::Move { unit, to }).unwrap();
    assert!(game.has_met(0, 1));
    assert!(game.has_met(1, 0));
    game.city_yields(theirs);
    assert_eq!(game.rival_yields_reused().0, before);
}

#[test]
fn a_village_reward_drops_rivals() {
    let (mut game, _, theirs, unit, to) = fixture();
    game.map.tiles.get_mut(&to).unwrap().improvement = Some(crate::name!("goody_hut"));
    game.city_yields(theirs);
    let before = game.rival_yields_reused().0;
    let mut fresh = game.clone();
    fresh.keep_rival_yields(false);
    let action = Action::Move { unit, to };
    game.apply(0, &action).unwrap();
    fresh.apply(0, &action).unwrap();
    assert_ne!(
        game.map.tiles[&to].improvement,
        Some(crate::name!("goody_hut"))
    );
    assert_eq!(
        bits(game.city_yields(theirs)),
        bits(fresh.city_yields(theirs))
    );
    assert_eq!(game.rival_yields_reused().0, before);
}

#[test]
fn refused_steps_drop_rivals() {
    let (mut game, _, theirs, unit, to) = fixture();
    game.units.get_mut(&unit).unwrap().moves_left = 0.0;
    game.city_yields(theirs);
    let before = game.rival_yields_reused().0;
    assert_eq!(
        game.apply(0, &Action::Move { unit, to }).unwrap_err(),
        "no moves left"
    );
    game.city_yields(theirs);
    assert_eq!(game.rival_yields_reused().0, before);
}

#[test]
fn envoy_values_are_read_back_even_when_the_ledger_length_is_unchanged() {
    let (mut game, _, _, unit, to) = fixture();
    game.players[1].envoys.push((0, 2));
    let before = game
        .quiet_move_yields(0, &Action::Move { unit, to })
        .unwrap();
    game.players[1].envoys[0].1 += 1;
    assert!(!game.quiet_move_yields_unchanged(&before));
}

#[test]
fn occupied_destinations_keep_the_conservative_action_path() {
    let (mut game, _, _, unit, to) = fixture();
    game.spawn_unit("builder", 1, to);
    assert!(game
        .quiet_move_yields(0, &Action::Move { unit, to })
        .is_none());
}

#[test]
fn an_invalid_destination_is_refused_without_geometry_lookup() {
    let (mut game, _, theirs, unit, _) = fixture();
    game.city_yields(theirs);
    let before = game.rival_yields_reused().0;
    assert!(game
        .apply(
            0,
            &Action::Move {
                unit,
                to: (i32::MAX, i32::MAX)
            }
        )
        .is_err());
    game.city_yields(theirs);
    assert_eq!(game.rival_yields_reused().0, before);
}
