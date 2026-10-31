use super::*;
use crate::game::Action;
use std::sync::Arc;

/// One founded capital, a Warrior three tiles out, no other unit; the
/// capital's Amenities pushed to `surplus`.
fn short_capital(surplus: i64) -> (Game, u32, u32) {
    let mut g = Game::new_full(2, 24, 16, 91_783, 250, 0, false);
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
    let base = g.city_amenity_surplus(&g.cities[&cid]);
    Arc::make_mut(&mut g.observed_city_amenity_adjustments).insert(cid, surplus - base);
    let post = g
        .map
        .tiles
        .values()
        .filter(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        .map(|tile| tile.pos)
        .filter(|pos| g.wdist(*pos, home) == 3)
        .min()
        .expect("fixture: land three tiles from the capital");
    let warrior = g.spawn_test_unit("warrior", 0, post);
    (g, cid, warrior)
}

fn armed() -> BasicAi {
    let mut ai = BasicAi::new();
    ai.enable_retainers_garrison_the_short_cities();
    ai
}

#[test]
fn a_short_city_draws_an_idle_soldier_while_retainers_is_slotted() {
    let (mut g, cid, warrior) = short_capital(-1);
    g.players[0].policies.insert(crate::name!("retainers"));
    assert!(g.policy_effect(0, "garrison_amenity") > 0.0);
    let start = g.wdist(g.units[&warrior].pos, g.cities[&cid].pos);
    let mut ai = armed();
    assert_eq!(ai.retainers_garrison_step(&mut g, 0, warrior), Some(true));
    assert_eq!(ai.retainers_garrison_claims.get(&cid), Some(&warrior));
    assert!(g.wdist(g.units[&warrior].pos, g.cities[&cid].pos) < start);
}

#[test]
fn no_retainers_or_no_shortfall_leaves_the_unit_alone() {
    // Without the card the step stands aside.
    let (mut g, _cid, warrior) = short_capital(-1);
    let mut ai = armed();
    assert_eq!(ai.retainers_garrison_step(&mut g, 0, warrior), None);

    // A city at zero does not start a claim.
    let (mut g, _cid, warrior) = short_capital(0);
    g.players[0].policies.insert(crate::name!("retainers"));
    let mut ai = armed();
    assert_eq!(ai.retainers_garrison_step(&mut g, 0, warrior), None);

    // Off, nothing moves.
    let (mut g, _cid, warrior) = short_capital(-1);
    g.players[0].policies.insert(crate::name!("retainers"));
    let mut ai = BasicAi::new();
    assert_eq!(ai.retainers_garrison_step(&mut g, 0, warrior), None);
}

#[test]
fn the_garrison_holds_the_city_it_made_content() {
    let (mut g, cid, warrior) = short_capital(-1);
    g.players[0].policies.insert(crate::name!("retainers"));
    let home = g.cities[&cid].pos;
    g.units.get_mut(&warrior).unwrap().pos = home;
    let mut ai = armed();
    ai.retainers_garrison_claims.insert(cid, warrior);
    // The garrison's own Amenity brings the city to zero: the claim holds.
    let base = g.city_amenity_surplus(&g.cities[&cid]);
    let adjust = g.observed_city_amenity_adjustments[&cid];
    Arc::make_mut(&mut g.observed_city_amenity_adjustments).insert(cid, adjust - base);
    assert_eq!(g.city_amenity_surplus(&g.cities[&cid]), 0);
    assert_eq!(ai.retainers_garrison_step(&mut g, 0, warrior), Some(false));
    assert_eq!(ai.retainers_garrison_claims.get(&cid), Some(&warrior));
}
