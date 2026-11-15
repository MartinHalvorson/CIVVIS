use super::super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

/// Two majors, each with a capital; seat 0 fields six swordsmen, seat 1 only
/// its starting units. The capitals stand within the declaration range.
fn open_board() -> Game {
    let mut game = Game::new_full(2, 24, 16, 7_951, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    for _ in 0..6 {
        game.spawn_test_unit("swordsman", 0, capital);
    }
    game
}

fn rival_capital(game: &Game) -> u32 {
    game.player_city_ids(1)[0]
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_open_capital_declares_at_twice();
    ai
}

#[test]
fn the_board_meets_the_gate() {
    let game = open_board();
    let capital = &game.cities[&rival_capital(&game)];
    assert!(capital.is_capital && capital.original_owner == 1);
    assert!(capital.wall_hp <= 0, "a new capital has no walls");
    assert!(AdvancedAi::city_within_declaration_range(
        &game,
        0,
        capital.pos
    ));
    assert!(
        game.military_power(0) >= super::OPEN_CAPITAL_EDGE * game.military_power(1).max(1.0),
        "six swordsmen hold twice the rival's starting units"
    );
}

#[test]
fn an_open_capital_at_twice_is_declared_on_only_under_the_gene_and_domination() {
    let game = open_board();
    let capital = rival_capital(&game);
    let off = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(off.open_capital_at_twice(&game, 0, 1), None, "off");
    assert!(!off.prey_is_declared_on_now(&game, 0, 1), "off");

    let ai = armed();
    assert_eq!(ai.open_capital_at_twice(&game, 0, 1), Some(capital));
    assert!(
        ai.prey_is_declared_on_now(&game, 0, 1),
        "it declares as on prey without `prey-is-declared-on`"
    );

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_open_capital_declares_at_twice();
    assert_eq!(
        science.open_capital_at_twice(&game, 0, 1),
        None,
        "Domination only"
    );
    assert!(!science.prey_is_declared_on_now(&game, 0, 1));
}

#[test]
fn a_walled_capital_is_not_declared_on() {
    let mut game = open_board();
    let capital = rival_capital(&game);
    game.cities.get_mut(&capital).unwrap().wall_hp = 100;
    assert_eq!(armed().open_capital_at_twice(&game, 0, 1), None);
}

#[test]
fn a_rival_over_half_our_power_is_not_declared_on() {
    let mut game = open_board();
    let pos = game.cities[&rival_capital(&game)].pos;
    for _ in 0..4 {
        game.spawn_test_unit("swordsman", 1, pos);
    }
    assert!(
        game.military_power(0) < super::OPEN_CAPITAL_EDGE * game.military_power(1),
        "the rival now holds more than half our power"
    );
    assert_eq!(armed().open_capital_at_twice(&game, 0, 1), None);
}

#[test]
fn a_rival_at_war_with_us_is_not_declared_on_again() {
    let mut game = open_board();
    // `Action::DeclareWar` is refused on this bare board, so the war is
    // written directly, as the engine's own tests do.
    game.at_war.insert((0, 1));
    assert!(game.is_at_war(0, 1));
    assert_eq!(armed().open_capital_at_twice(&game, 0, 1), None);
}
