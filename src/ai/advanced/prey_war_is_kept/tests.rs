use super::super::faith_war_before_the_flip::FaithWar;
use super::super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

/// Two majors, each with a capital, at war; seat 0 fields an army, seat 1
/// its starting units.
fn prey_at_war() -> Game {
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
    game.record_contact(0, 1);
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    for _ in 0..6 {
        game.spawn_test_unit("swordsman", 0, capital);
    }
    game.turn = 90;
    game.at_war.insert((0, 1));
    game
}

#[test]
fn a_war_on_prey_is_kept_only_under_the_gene_and_domination() {
    let game = prey_at_war();
    assert!(
        game.military_power(1) <= super::PREY_DECLARE_RATIO * game.military_power(0),
        "fixture: seat 1 is prey"
    );
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.prey_war_is_kept_with(&game, 0, 1), "off");
    ai.enable_prey_war_is_kept();
    assert!(ai.prey_war_is_kept_with(&game, 0, 1));

    let mut other = AdvancedAi::new();
    other.enable_prey_war_is_kept();
    assert!(
        !other.prey_war_is_kept_with(&game, 0, 1),
        "no Domination target"
    );
}

#[test]
fn a_rival_near_our_power_or_at_peace_is_not_kept() {
    let mut game = prey_at_war();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_prey_war_is_kept();
    game.at_war.remove(&(0, 1));
    assert!(!ai.prey_war_is_kept_with(&game, 0, 1), "at peace");

    game.at_war.insert((0, 1));
    let capital = game.cities[&game.player_city_ids(1)[0]].pos;
    for _ in 0..5 {
        game.spawn_test_unit("swordsman", 1, capital);
    }
    assert!(!ai.prey_war_is_kept_with(&game, 0, 1), "not prey");
}

#[test]
fn a_faith_war_on_prey_stays_a_front() {
    let mut game = prey_at_war();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_faith_war_is_no_front();
    ai.faith_war_before_the_flip_war = Some(FaithWar {
        rival: 1,
        opened: 88,
        spreader_seen: 88,
    });
    assert_eq!(ai.no_front_rivals(&game, 0), vec![1], "gene off: no front");

    ai.enable_prey_war_is_kept();
    assert!(ai.no_front_rivals(&game, 0).is_empty());
    assert_eq!(ai.without_faith_war(&game, 0, vec![1]), vec![1]);

    // A rival grown back to our power is exempt again.
    let capital = game.cities[&game.player_city_ids(1)[0]].pos;
    for _ in 0..5 {
        game.spawn_test_unit("swordsman", 1, capital);
    }
    assert_eq!(ai.no_front_rivals(&game, 0), vec![1], "not prey");
}
