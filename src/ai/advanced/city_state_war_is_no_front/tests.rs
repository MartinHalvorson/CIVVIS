use super::super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

/// Seat 0 and major 1 at peace, each with a capital; player 2 a city-state
/// with a city, at war with seat 0.
fn city_state_war() -> Game {
    let mut game = Game::new_full(3, 24, 16, 7_971, 300, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every seat starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    game.players[2].is_minor = true;
    for pid in 0..3 {
        for other in pid + 1..3 {
            game.record_contact(pid, other);
        }
    }
    game.turn = 90;
    game.at_war.insert((0, 2));
    game
}

#[test]
fn a_city_state_war_is_no_front_only_for_domination_under_the_gene() {
    let game = city_state_war();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(ai.without_city_state_war(&game, vec![2]), vec![2], "off");

    ai.enable_city_state_war_is_no_front();
    assert!(ai.without_city_state_war(&game, vec![2]).is_empty());
    assert_eq!(
        ai.without_city_state_war(&game, vec![1, 2]),
        vec![1],
        "a major war stays a front"
    );

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_city_state_war_is_no_front();
    assert_eq!(
        science.without_city_state_war(&game, vec![2]),
        vec![2],
        "only a Domination seat"
    );
}

#[test]
fn the_plan_does_not_aim_a_domination_campaign_at_the_city_state() {
    let game = city_state_war();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let off = ai.assess(&game, 0);
    assert_eq!(
        off.target_player,
        Some(2),
        "off, the city-state war is the front"
    );

    ai.enable_city_state_war_is_no_front();
    let on = ai.assess(&game, 0);
    assert_ne!(on.target_player, Some(2));
}
