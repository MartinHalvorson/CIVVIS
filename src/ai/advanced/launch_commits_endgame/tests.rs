use super::*;

/// Two majors, our capital founded, the Science victory enabled.
fn board() -> Game {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    game.victory_conditions.science = true;
    game.turn = 180;
    game
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_launch_commits_the_endgame();
    }
    ai
}

fn launched(mut game: Game) -> Game {
    game.players[0]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    game
}

/// A Religion seat whose Earth Satellite has landed is committed to the
/// science endgame with the gene; without it, it is not.
#[test]
fn our_satellite_commits_an_assigned_religion_seat() {
    let game = launched(board());
    let on = seat(VictoryTarget::Religion, true);
    assert!(on.victory_planning, "fixture: a planning seat");
    assert!(on.our_launch_commits_the_endgame(&game, 0));
    assert!(on.science_endgame_committed(&game, 0));
    assert!(on.science_endgame_production_committed(&game, 0));

    let off = seat(VictoryTarget::Religion, false);
    assert!(!off.science_endgame_committed(&game, 0));
    assert!(!off.science_endgame_production_committed(&game, 0));
}

/// Before our first launch nothing changes, and a Domination seat stays out
/// of the endgame either way.
#[test]
fn no_launch_and_domination_stay_out() {
    let before = board();
    let on = seat(VictoryTarget::Culture, true);
    assert!(!on.our_launch_commits_the_endgame(&before, 0));
    assert!(!on.science_endgame_committed(&before, 0));

    let game = launched(board());
    let domination = seat(VictoryTarget::Domination, true);
    assert!(!domination.science_endgame_committed(&game, 0));
}
