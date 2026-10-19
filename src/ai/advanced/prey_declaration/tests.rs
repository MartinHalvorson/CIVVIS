use super::super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

/// Two majors, each with a capital; seat 0 fields an army, seat 1 a lone
/// warrior.
fn prey_board() -> Game {
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

#[test]
fn a_weak_major_is_prey_only_under_the_gene_and_domination() {
    let game = prey_board();
    assert!(game.military_power(1) <= super::PREY_DECLARE_RATIO * game.military_power(0));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.prey_is_declared_on_now(&game, 0, 1), "off");
    ai.enable_prey_is_declared_on();
    assert!(ai.prey_is_declared_on_now(&game, 0, 1));

    let mut other = AdvancedAi::new();
    other.enable_prey_is_declared_on();
    assert!(
        !other.prey_is_declared_on_now(&game, 0, 1),
        "no Domination target"
    );
}

#[test]
fn a_rival_near_our_power_is_not_prey() {
    let mut game = prey_board();
    let capital = game.cities[&game.player_city_ids(1)[0]].pos;
    for _ in 0..5 {
        game.spawn_test_unit("swordsman", 1, capital);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_prey_is_declared_on();
    assert!(!ai.prey_is_declared_on_now(&game, 0, 1));
}
