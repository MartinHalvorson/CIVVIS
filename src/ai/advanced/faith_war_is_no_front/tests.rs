use super::super::faith_war_before_the_flip::FaithWar;
use super::super::AdvancedAi;
use crate::game::Game;

/// Three majors in contact, each with a capital; seat 0 is at war with
/// major 1, the faith war's rival.
fn faith_war_on_one() -> (Game, AdvancedAi) {
    let mut game = Game::new_full(3, 24, 16, 7_941, 300, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    for pid in 0..3 {
        for other in pid + 1..3 {
            game.record_contact(pid, other);
        }
    }
    game.turn = 90;
    game.at_war.insert((0, 1));
    let mut ai = AdvancedAi::new();
    ai.enable_one_war_at_a_time();
    ai.faith_war_before_the_flip_war = Some(FaithWar {
        rival: 1,
        opened: 88,
        spreader_seen: 88,
    });
    (game, ai)
}

#[test]
fn a_faith_war_holds_no_declaration_only_under_the_gene() {
    let (game, mut ai) = faith_war_on_one();
    assert_eq!(ai.one_war_enemies(&game, 0), vec![1]);
    assert_eq!(ai.without_faith_war(&game, 0, vec![1]), vec![1], "off");
    assert!(
        ai.one_war_holds_declaration(&game, 0, 2),
        "off, the faith war is the war being fought"
    );

    ai.enable_faith_war_is_no_front();
    assert_eq!(ai.faith_war_front_exempt(&game, 0), Some(1));
    assert!(ai.without_faith_war(&game, 0, vec![1]).is_empty());
    assert!(!ai.one_war_holds_declaration(&game, 0, 2));
}

#[test]
fn another_war_still_holds_and_a_closed_faith_war_exempts_nobody() {
    let (mut game, mut ai) = faith_war_on_one();
    ai.enable_faith_war_is_no_front();
    // A real war with major 2 beside the faith war.
    game.at_war.insert((0, 2));
    assert_eq!(ai.without_faith_war(&game, 0, vec![1, 2]), vec![2]);

    // Peace with major 1: nothing is exempt any more.
    game.at_war.remove(&(0, 1));
    assert_eq!(ai.faith_war_front_exempt(&game, 0), None);
    assert_eq!(ai.without_faith_war(&game, 0, vec![2]), vec![2]);
}
