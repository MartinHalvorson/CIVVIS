use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// Seat 0 with a capital at (3, 6), a Settler at (8, 6) and a barbarian
/// warrior beside it at (9, 6); open grassland everywhere.
fn board() -> (Game, AdvancedAi, u32, Pos) {
    let mut g = Game::new_full(2, 24, 16, 7925, 250, 1, true);
    g.current = 0;
    g.clear_mirror_cities();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.river_edges = [true; 6];
    }
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let capital = g.spawn_test_unit("settler", 0, (3, 6));
    g.apply(0, &Action::FoundCity { unit: capital }).unwrap();
    let here = (8, 6);
    let uid = g.spawn_test_unit("settler", 0, here);
    g.spawn_test_unit("warrior", g.barb_pid.unwrap(), (9, 6));
    let mut ai = AdvancedAi::new();
    ai.settlement_safety = true;
    ai.settler_founds_when_stalled = true;
    (g, ai, uid, here)
}

#[test]
fn settler_founds_under_raiders_is_opt_in() {
    opt_in_off_in_both_controllers("settler-founds-under-raiders", |ai| {
        ai.settler_founds_under_raiders
    });
}

/// The stall fallback refuses the raider's tile risk stock; under the gene a
/// city replaces the Settler, since a barbarian never takes a city.
#[test]
fn a_stalled_settler_among_raiders_founds_in_place() {
    let (mut g, mut ai, uid, here) = board();
    assert!(g.can_found_city(uid));
    let visible = ai.battlefront_visibility(&g, 0);
    assert!(
        ai.settlement_tile_risk(&g, 0, Some(uid), here, &visible) > SETTLER_STEP_RISK_LIMIT,
        "fixture: the raider makes the tile risky"
    );
    let mut stock = g.clone();
    assert!(!ai.founds_where_it_stands(&mut stock, 0, uid, here));
    assert!(stock.city_at(here).is_none(), "stock: the risk refuses");

    ai.enable_settler_founds_under_raiders();
    assert!(ai.settler_threat_is_raiders_only(&g, 0, here, &visible));
    assert!(ai.founds_where_it_stands(&mut g, 0, uid, here));
    assert!(
        g.city_at(here).is_some(),
        "the city replaces the exposed settler"
    );
}

/// A major we are at war with in reach keeps the refusal: its army does take
/// cities.
#[test]
fn a_major_at_war_in_reach_keeps_the_refusal() {
    let (mut g, mut ai, uid, here) = board();
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.spawn_test_unit("warrior", 1, (8, 8));
    ai.enable_settler_founds_under_raiders();
    let visible = ai.battlefront_visibility(&g, 0);
    assert!(!ai.settler_threat_is_raiders_only(&g, 0, here, &visible));
    assert!(!ai.founds_where_it_stands(&mut g, 0, uid, here));
    assert!(g.city_at(here).is_none());
}

/// No hostile in reach is no raider threat (the tile risk itself is then low).
#[test]
fn no_hostile_is_no_raider_threat() {
    let (mut g, mut ai, _uid, here) = board();
    let raiders: Vec<u32> = g
        .units
        .values()
        .filter(|unit| Some(unit.owner) == g.barb_pid)
        .map(|unit| unit.id)
        .collect();
    for raider in raiders {
        g.remove_unit(raider);
    }
    ai.enable_settler_founds_under_raiders();
    let visible = ai.battlefront_visibility(&g, 0);
    assert!(!ai.settler_threat_is_raiders_only(&g, 0, here, &visible));
}
