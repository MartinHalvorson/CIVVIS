use super::*;

#[test]
fn repeated_planning_frames_do_not_retire_a_scout_goal_early() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("DEADGOAL", 91_779),
        250,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let scout = game.spawn_unit("scout", 0, home);
    game.units.get_mut(&scout).unwrap().moves_left = 0.0;
    game.turn = 10;
    let mut ai = BasicAi::new();
    ai.enable_explore_dead_targets();
    let goal = ai.exploration_goal(&game, 0, scout, false).unwrap();
    for elapsed in 0..EXPLORE_STUCK_TURNS {
        game.turn = 10 + elapsed;
        for _ in 0..5 {
            let _ = ai.explore_step(&mut game, 0, scout);
            assert!(
                ai.explore_dead
                    .borrow()
                    .get(&scout)
                    .is_none_or(|dead| dead.is_empty()),
                "planning again in turn {} is not another unmoved game turn",
                game.turn
            );
        }
    }
    game.turn = 10 + EXPLORE_STUCK_TURNS;
    let _ = ai.explore_step(&mut game, 0, scout);
    assert_eq!(
        ai.explore_dead.borrow()[&scout][&goal],
        game.turn + EXPLORE_DEAD_TARGET_TURNS
    );
}
