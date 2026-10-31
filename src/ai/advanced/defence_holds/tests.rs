use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn walls() -> Item {
    Item::Building {
        building: crate::name!("walls"),
    }
}

fn amphitheater() -> Item {
    Item::Building {
        building: crate::name!("amphitheater"),
    }
}

fn archer() -> Item {
    Item::Unit {
        unit: crate::name!("archer"),
    }
}

/// Our city at (10, 10) and a rival's city far off, no units on the board.
fn board() -> (Game, u32) {
    let mut g = Game::new_full(2, 40, 24, 936_811, 650, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let city = g.found_city_for(0, (10, 10), None);
    g.found_city_for(1, (32, 10), None);
    g.current = 0;
    g.turn = 60;
    (g, city)
}

#[test]
fn defence_holds_under_fire_is_opt_in() {
    opt_in_off_in_both_controllers("defence-holds-under-fire", |ai| ai.defence_holds_under_fire);
}

/// An enemy soldier beside a city at war holds its walls against an
/// Amphitheater, not against an archer; at peace, with no soldier near, or
/// with the gene off, nothing holds.
#[test]
fn walls_hold_against_a_civilian_build_under_fire() {
    let (mut g, city) = board();
    let mut ai = AdvancedAi::new();
    ai.enable_defence_holds_under_fire();
    assert!(
        !ai.defence_holds_under_fire(&g, 0, city, &walls(), &amphitheater()),
        "no soldier near"
    );
    g.spawn_test_unit("warrior", 1, (11, 10));
    assert!(
        !ai.city_under_fire_for_the_hold(&g, 0, city),
        "at peace, the soldier is no threat"
    );
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    assert!(
        ai.city_under_fire_for_the_hold(&g, 0, city),
        "fixture: at war, beside the city"
    );
    assert!(ai.defence_holds_under_fire(&g, 0, city, &walls(), &amphitheater()));
    assert!(
        !ai.defence_holds_under_fire(&g, 0, city, &walls(), &archer()),
        "a defender may replace the walls"
    );
    assert!(
        !ai.defence_holds_under_fire(&g, 0, city, &amphitheater(), &walls()),
        "only a defensive commitment holds"
    );
    assert!(
        !AdvancedAi::new().defence_holds_under_fire(&g, 0, city, &walls(), &amphitheater()),
        "gene off"
    );
}

/// A damaged city holds without a visible soldier.
#[test]
fn a_damaged_city_holds() {
    let (mut g, city) = board();
    let mut ai = AdvancedAi::new();
    ai.enable_defence_holds_under_fire();
    g.cities.get_mut(&city).unwrap().hp = 150;
    assert!(ai.defence_holds_under_fire(&g, 0, city, &walls(), &amphitheater()));
}
