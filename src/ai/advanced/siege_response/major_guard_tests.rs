use super::{tests::open_ring, tests::siege_board, *};

fn guarded_pair(seed: u64, field: bool, ranged: bool) -> (Game, AdvancedAi, u32, u32, u32) {
    let (mut game, _, home) = siege_board(seed);
    let post = if field {
        open_ring(&game, home, 2)[0]
    } else {
        home
    };
    let settler = game.spawn_test_unit("settler", 0, post);
    let guard = game.spawn_test_unit(if ranged { "archer" } else { "warrior" }, 0, post);
    let enemy = game.spawn_test_unit("warrior", 1, open_ring(&game, post, 1)[0]);
    let mut ai = AdvancedAi::new();
    ai.enable_guard_breaks_the_pin();
    ai.settler_guards.insert(settler, guard);
    (game, ai, settler, guard, enemy)
}

#[test]
fn ranged_settler_guard_fires_on_wartime_major_without_leaving_the_city() {
    let (mut game, mut ai, settler, guard, enemy) = guarded_pair(90_211, false, true);
    game.at_war.insert((0, 1));
    // A major-war shot must not depend on a Barbarian actor existing.
    game.barb_pid = None;
    let post = game.units[&guard].pos;
    assert_eq!(ai.stacked_guard_step(&mut game, 0, guard), Some(true));
    assert!(
        game.units[&enemy].hp < 100,
        "the city guard actually shoots"
    );
    assert_eq!(game.units[&guard].pos, post);
    assert_eq!(game.units[&settler].pos, post);
    assert_eq!(ai.settler_guards.get(&settler), Some(&guard));
}

#[test]
fn ranged_settler_guard_also_keeps_its_field_post_while_firing() {
    let (mut game, mut ai, settler, guard, enemy) = guarded_pair(90_212, true, true);
    game.at_war.insert((0, 1));
    let post = game.units[&guard].pos;
    assert_eq!(ai.stacked_guard_step(&mut game, 0, guard), Some(true));
    assert!(game.units[&enemy].hp < 100);
    assert_eq!(game.units[&guard].pos, post);
    assert_eq!(game.units[&settler].pos, post);
}

#[test]
fn ranged_settler_guard_requires_war_and_an_available_legal_shot() {
    let (game, ai, _, guard, enemy) = guarded_pair(90_213, false, true);
    for case in [
        "peace",
        "disabled",
        "spent_attack",
        "no_moves",
        "out_of_range",
    ] {
        let mut board = game.clone();
        let mut controller = ai.clone();
        if case != "peace" {
            board.at_war.insert((0, 1));
        }
        match case {
            "disabled" => controller.disable_guard_breaks_the_pin(),
            "spent_attack" => board.units.get_mut(&guard).unwrap().attacks_left = 0,
            "no_moves" => board.units.get_mut(&guard).unwrap().moves_left = 0.0,
            "out_of_range" => {
                let post = board.units[&guard].pos;
                board.units.get_mut(&enemy).unwrap().pos = open_ring(&board, post, 4)[0];
            }
            _ => {}
        }
        controller.stacked_guard_step(&mut board, 0, guard);
        assert_eq!(board.units[&enemy].hp, 100, "{case}");
    }
}

#[test]
fn melee_settler_guard_does_not_gain_a_major_war_charge() {
    let (mut game, mut ai, settler, guard, enemy) = guarded_pair(90_214, true, false);
    game.at_war.insert((0, 1));
    game.units.get_mut(&enemy).unwrap().hp = 1;
    let post = game.units[&guard].pos;
    ai.stacked_guard_step(&mut game, 0, guard);
    assert_eq!(game.units[&enemy].hp, 1);
    assert_eq!(game.units[&guard].pos, post);
    assert_eq!(game.units[&settler].pos, post);
}
