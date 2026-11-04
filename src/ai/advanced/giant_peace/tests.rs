use super::super::AdvancedAi;
use crate::game::Game;

/// Two majors at war, each with a capital; seat 1 fields an army, and seat 0
/// founded a second city that seat 1 now holds.
fn giant_war() -> (Game, u32) {
    let mut game = Game::new_full(2, 24, 16, 7_971, 300, 0, false);
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
    game.at_war.insert((0, 1));
    let theirs = game.cities[&game.player_city_ids(1)[0]].pos;
    for _ in 0..8 {
        game.spawn_test_unit("swordsman", 1, theirs);
    }
    let lost = game.player_city_ids(0)[0];
    (game, lost)
}

#[test]
fn a_giant_holding_our_city_gets_the_offer_only_under_the_gene() {
    let (mut game, lost) = giant_war();
    assert!(
        game.military_power(1) >= super::GIANT_PEACE_RATIO * game.military_power(0).max(1.0),
        "fixture: seat 1 is a giant"
    );
    let mut ai = AdvancedAi::new();
    ai.enable_the_giant_gets_the_peace_offer();
    assert!(
        !ai.giant_gets_the_peace_offer_from(&game, 0, 1),
        "a giant that has taken nothing of ours"
    );
    // Seat 1 takes the city seat 0 founded.
    game.cities.get_mut(&lost).unwrap().owner = 1;
    assert!(ai.giant_gets_the_peace_offer_from(&game, 0, 1));
    assert!(
        !AdvancedAi::new().giant_gets_the_peace_offer_from(&game, 0, 1),
        "off"
    );
    game.at_war.remove(&(0, 1));
    assert!(!ai.giant_gets_the_peace_offer_from(&game, 0, 1), "at peace");
}

#[test]
fn a_rival_under_the_ratio_is_no_giant() {
    let (mut game, lost) = giant_war();
    let ours = game.cities[&lost].pos;
    for _ in 0..8 {
        game.spawn_test_unit("swordsman", 0, ours);
    }
    game.cities.get_mut(&lost).unwrap().owner = 1;
    let mut ai = AdvancedAi::new();
    ai.enable_the_giant_gets_the_peace_offer();
    assert!(!ai.giant_gets_the_peace_offer_from(&game, 0, 1));
}
