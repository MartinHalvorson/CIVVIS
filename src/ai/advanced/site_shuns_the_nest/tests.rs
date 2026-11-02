use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// A two-major game with our capital founded and every barbarian camp
/// cleared; returns the game and a land tile `distance` away from a camp
/// placed beside the capital's ring.
fn board() -> (Game, Pos) {
    let mut g = Game::new_full(2, 40, 24, 936_731, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.barb_camps.clear();
    let capital = g.player_city_ids(0)[0];
    let home = g.cities[&capital].pos;
    let camp = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| g.wdist(**pos, home) == 7 && !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .min()
        .expect("fixture: a land tile seven from the capital");
    g.barb_camps.insert(camp, g.turn + 1_000);
    (g, camp)
}

fn tile_at(g: &Game, from: Pos, distance: i32) -> Pos {
    g.map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.wdist(*pos, from) == distance)
        .min()
        .expect("fixture: a tile at the distance")
}

#[test]
fn site_shuns_the_nest_is_opt_in() {
    opt_in_off_in_both_controllers("site-shuns-the-nest", |ai| ai.site_shuns_the_nest);
}

/// The penalty steps down with the camp's distance and is zero beyond 4 and
/// with the gene off.
#[test]
fn the_nest_penalty_follows_the_camp_distance() {
    let (g, camp) = board();
    let stock = AdvancedAi::new();
    let mut ai = AdvancedAi::new();
    ai.enable_site_shuns_the_nest();
    for (distance, expected) in [
        (1, NEST_PENALTY_ADJACENT),
        (2, NEST_PENALTY_ADJACENT),
        (3, NEST_PENALTY_NEAR),
        (4, NEST_PENALTY_EDGE),
        (5, 0.0),
    ] {
        let pos = tile_at(&g, camp, distance);
        assert_eq!(ai.nest_penalty(&g, pos), expected, "distance {distance}");
        assert_eq!(stock.nest_penalty(&g, pos), 0.0, "off at {distance}");
    }
}

/// The site's safety penalty carries the nest price under the gene.
#[test]
fn the_safety_penalty_carries_the_nest() {
    let (g, camp) = board();
    let pos = tile_at(&g, camp, 2);
    let visible = g.player_vision_frame(0);
    let stock = AdvancedAi::new();
    let mut ai = AdvancedAi::new();
    ai.enable_site_shuns_the_nest();
    let off = stock.settlement_safety_penalty(&g, 0, pos, &visible);
    let on = ai.settlement_safety_penalty(&g, 0, pos, &visible);
    assert!(
        (on - off - NEST_PENALTY_ADJACENT).abs() < 1e-9,
        "on {on} off {off}"
    );
}
