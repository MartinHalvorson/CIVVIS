use super::super::AdvancedAi;
use crate::game::Game;

/// Two majors at war, seat 0 with a capital and an Archer beside it; no
/// hostile near yet. Returns the board, the Archer and the capital's tile.
fn walls() -> (Game, u32, crate::Pos) {
    let mut game = Game::new_full(2, 24, 16, 7_961, 300, 0, false);
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
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    let beside = game
        .nbrs(capital)
        .into_iter()
        .find(|pos| game.unit_ids_at(*pos).is_empty())
        .expect("a free tile beside the capital");
    let archer = game.spawn_test_unit("archer", 0, beside);
    (game, archer, capital)
}

/// A free tile exactly `distance` from `from`.
fn free_at(game: &Game, from: crate::Pos, distance: i32) -> crate::Pos {
    game.wdisk(from, distance)
        .into_iter()
        .filter(|pos| game.wdist(*pos, from) == distance && game.unit_ids_at(*pos).is_empty())
        .min()
        .expect("a free tile at that distance")
}

#[test]
fn a_defender_holds_only_under_the_gene_and_a_threat() {
    let (mut game, archer, capital) = walls();
    let mut ai = AdvancedAi::new();
    ai.enable_defenders_hold_the_walls();
    assert!(
        !ai.holds_the_walls(&game, 0, archer),
        "no hostile near the walls"
    );

    let raider = free_at(&game, capital, super::WALL_THREAT_REACH);
    game.spawn_test_unit("warrior", 1, raider);
    assert!(ai.holds_the_walls(&game, 0, archer));
    assert!(!AdvancedAi::new().holds_the_walls(&game, 0, archer), "off");

    // A hostile at peace is no threat.
    game.at_war.remove(&(0, 1));
    assert!(!ai.holds_the_walls(&game, 0, archer), "at peace");
}

#[test]
fn the_wounded_and_the_distant_do_not_hold() {
    let (mut game, archer, capital) = walls();
    let raider = free_at(&game, capital, super::WALL_THREAT_REACH);
    game.spawn_test_unit("warrior", 1, raider);
    let mut ai = AdvancedAi::new();
    ai.enable_defenders_hold_the_walls();

    let far = free_at(&game, capital, super::WALL_DEFENDER_REACH + 1);
    let distant = game.spawn_test_unit("archer", 0, far);
    assert!(!ai.holds_the_walls(&game, 0, distant), "beyond the reach");

    game.units.get_mut(&archer).unwrap().hp = super::ROTATE_HP - 1;
    assert!(!ai.holds_the_walls(&game, 0, archer), "wounded");
}
