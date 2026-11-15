use super::super::{AdvancedAi, VictoryTarget};
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::Game;

/// Our city and rival 1's capital six tiles apart on one grassland island,
/// rival 2's capital on another; ocean everywhere else, so the map's wrap
/// cannot join them. We field six swordsmen; the rivals field nothing.
fn open_board() -> Game {
    let mut g = Game::new_full(3, 40, 24, 936_611, 650, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.feature = None;
        tile.hills = false;
        let (x, y) = tile.pos;
        let land = (8..=16).contains(&y) && ((3..=18).contains(&x) || (26..=35).contains(&x));
        tile.terrain = if land {
            crate::name!("grassland")
        } else {
            crate::name!("ocean")
        };
    }
    let home = g.found_city_for(0, (10, 12), None);
    g.found_city_for(1, (16, 12), None);
    g.found_city_for(2, (30, 12), None);
    for pid in 0..3 {
        for other in 0..3 {
            if pid != other {
                g.players[pid].met.insert(other);
            }
        }
        g.players[pid].explored.extend(g.map.tiles.keys().copied());
    }
    let pos = g.cities[&home].pos;
    for _ in 0..6 {
        g.spawn_test_unit("swordsman", 0, pos);
    }
    g.current = 0;
    g.turn = 100;
    g
}

fn capital_of(game: &Game, pid: usize) -> u32 {
    game.player_city_ids(pid)[0]
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_open_capital_declares_at_twice();
    ai
}

#[test]
fn open_capital_declares_at_twice_is_opt_in() {
    opt_in_off_in_both_controllers("open-capital-declares-at-twice", |ai| {
        ai.open_capital_declares_at_twice
    });
}

#[test]
fn the_board_meets_the_gate() {
    let game = open_board();
    let capital = &game.cities[&capital_of(&game, 1)];
    assert!(capital.is_capital && capital.original_owner == 1);
    assert!(capital.wall_hp <= 0, "a new capital has no walls");
    assert!(AdvancedAi::city_within_declaration_range(
        &game,
        0,
        capital.pos
    ));
    let ai = armed();
    assert!(ai.rival_reachable_by_land(&game, 0, 1), "one island");
    assert!(ai.campaign_target_legal(&game, 0, 1));
    assert!(
        game.military_power(0) >= super::OPEN_CAPITAL_EDGE * game.military_power(1).max(1.0),
        "six swordsmen against none"
    );
}

#[test]
fn an_open_capital_at_twice_is_declared_on_only_under_the_gene_and_domination() {
    let game = open_board();
    let capital = capital_of(&game, 1);
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
fn an_overseas_capital_out_of_range_is_not_declared_on() {
    let game = open_board();
    let ai = armed();
    assert!(
        !ai.rival_reachable_by_land(&game, 0, 2),
        "fixture: overseas"
    );
    assert_eq!(ai.open_capital_at_twice(&game, 0, 2), None);
}

#[test]
fn a_walled_capital_is_not_declared_on() {
    let mut game = open_board();
    let capital = capital_of(&game, 1);
    game.cities.get_mut(&capital).unwrap().wall_hp = 100;
    assert_eq!(armed().open_capital_at_twice(&game, 0, 1), None);
}

#[test]
fn a_rival_over_half_our_power_is_not_declared_on() {
    let mut game = open_board();
    let pos = game.cities[&capital_of(&game, 1)].pos;
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
    game.at_war.insert((0, 1));
    assert!(game.is_at_war(0, 1));
    assert_eq!(armed().open_capital_at_twice(&game, 0, 1), None);
}
