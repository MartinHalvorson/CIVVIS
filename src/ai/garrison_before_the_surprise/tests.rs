use super::*;
use crate::game::Action;

/// Our founded capital with no soldier in it, our Warrior six tiles out, and
/// a met major's soldiers two tiles from the capital: `rival_soldiers` of
/// them, so the rival's power against ours is the fixture's knob.
fn threatened_capital(rival_soldiers: usize) -> (Game, Pos, u32) {
    let mut g = Game::new_full(2, 24, 16, 91_784, 250, 0, false);
    g.current = 0;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    let cid = g.player_city_ids(0)[0];
    let home = g.cities[&cid].pos;
    let land_at = |g: &Game, distance: i32| {
        g.map
            .tiles
            .values()
            .filter(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            .map(|tile| tile.pos)
            .filter(|pos| g.wdist(*pos, home) == distance)
            .collect::<Vec<_>>()
    };
    let post = *land_at(&g, 6).first().expect("fixture: land six tiles out");
    let warrior = g.spawn_test_unit("warrior", 0, post);
    let door = land_at(&g, 2);
    assert!(door.len() >= rival_soldiers, "fixture: room for the rival");
    for pos in door.iter().take(rival_soldiers) {
        g.spawn_test_unit("warrior", 1, *pos);
    }
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    (g, home, warrior)
}

fn armed() -> BasicAi {
    let mut ai = BasicAi::new();
    ai.enable_garrison_before_the_surprise();
    ai
}

#[test]
fn a_stronger_neighbour_at_the_door_drafts_the_soldier_home() {
    let (mut g, home, warrior) = threatened_capital(3);
    assert!(g.military_power(1) >= GARRISON_CALL_RATIO * g.military_power(0));
    let mut ai = armed();
    assert_eq!(ai.garrison_called_cities(&g, 0), vec![home]);
    let start = g.wdist(g.units[&warrior].pos, home);
    assert_eq!(
        ai.garrison_before_the_surprise_step(&mut g, 0, warrior),
        Some(true)
    );
    assert!(g.wdist(g.units[&warrior].pos, home) < start);
}

#[test]
fn off_or_an_even_neighbour_leaves_the_soldier_alone() {
    let (mut g, _home, warrior) = threatened_capital(3);
    let mut off = BasicAi::new();
    assert_eq!(
        off.garrison_before_the_surprise_step(&mut g, 0, warrior),
        None
    );
    // One rival Warrior against our one: no call.
    let (mut g, _home, warrior) = threatened_capital(1);
    assert!(g.military_power(1) < GARRISON_CALL_RATIO * g.military_power(0));
    assert_eq!(
        armed().garrison_before_the_surprise_step(&mut g, 0, warrior),
        None
    );
}

#[test]
fn an_unmet_neighbour_calls_nothing() {
    let (mut g, _home, warrior) = threatened_capital(3);
    g.players[0].met.remove(&1);
    g.players[1].met.remove(&0);
    let mut ai = armed();
    assert!(ai.garrison_called_cities(&g, 0).is_empty());
    assert_eq!(
        ai.garrison_before_the_surprise_step(&mut g, 0, warrior),
        None
    );
}

#[test]
fn the_garrison_holds_once_home() {
    let (mut g, home, walker) = threatened_capital(3);
    g.remove_unit(walker);
    let warrior = g.spawn_test_unit("warrior", 0, home);
    let mut ai = armed();
    assert_eq!(
        ai.garrison_before_the_surprise_step(&mut g, 0, warrior),
        Some(true)
    );
    assert_eq!(g.units[&warrior].pos, home);
}
