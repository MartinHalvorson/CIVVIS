use super::*;

#[test]
fn unordered_attack_flood_matches_the_ordered_movement_contract() {
    let mut game = Game::new_full(2, 32, 22, 8_181, 300, 0, false);
    game.at_war.insert(pair(0, 1));
    let ours = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| {
            let spec = &game.rules.units[game.units[uid].kind];
            spec.class == "military" && (spec.is_melee_capable() || spec.has_ranged_attack())
        })
        .expect("a new major begins with a combat unit");
    let home = game.units[&ours].pos;
    let dry_empty = |game: &Game, pos: Pos| {
        game.map
            .get(pos)
            .is_some_and(|tile| !game.rules.is_water(tile))
            && game.unit_ids_at(pos).is_empty()
    };
    let enemy_pos = game
        .nbrs(home)
        .into_iter()
        .find(|pos| dry_empty(&game, *pos))
        .expect("the starting combat unit has an open adjacent land tile");
    let enemy = game.spawn_test_unit("warrior", 1, enemy_pos);
    let archer_pos = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, home) > 8 && dry_empty(&game, *pos))
        .min_by_key(|pos| (game.wdist(*pos, home), *pos))
        .expect("the fixture offers a distant open land tile");
    let archer = game.spawn_test_unit("archer", 0, archer_pos);

    for uid in [ours, enemy, archer] {
        let start = game.units[&uid].pos;
        let moves = game.unit_max_moves(uid);
        let expected: Vec<(Pos, f64)> = game
            .flow_past(uid, start, moves, true)
            .into_iter()
            .collect();
        let mut actual = game.flow_past_unordered(uid, start, moves, true);
        actual.sort_unstable_by_key(|(position, _)| *position);
        assert_eq!(
            actual, expected,
            "the streamed flood changed the movement kept by unit {uid}"
        );

        let (_, mut flood) = game.attack_reach_from_flood(uid);
        flood.sort_unstable();
        assert_eq!(
            flood,
            expected
                .into_iter()
                .map(|(position, _)| position)
                .collect::<Vec<_>>(),
            "attack reach no longer receives this unit's complete movement flood"
        );
    }
}
