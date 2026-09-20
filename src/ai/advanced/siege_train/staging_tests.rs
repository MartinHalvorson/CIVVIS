use super::tests::{plan_against, walled_city};
use super::*;

fn crowded_approach() -> (Game, u32, u32, u32, Pos) {
    let (mut g, cid) = walled_city();
    let target = g.cities[&cid].pos;
    let start = (target.0 - 7, target.1);
    let occupied = (target.0 - 6, target.1);
    let landing = (target.0 - 5, target.1);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if tile.pos.1 == target.1 && (start.0..=target.0).contains(&tile.pos.0) {
            crate::name!("grassland")
        } else {
            crate::name!("mountain")
        };
        tile.feature = None;
        tile.hills = false;
    }
    let gun = g.spawn_unit("catapult", 0, start);
    let screen = g.spawn_unit("swordsman", 0, occupied);
    assert!(!g.can_move(gun, occupied));
    assert_eq!(g.route_step(gun, target, STAGING_FAR), None);
    assert_eq!(
        g.pass_through_destination(gun, target, STAGING_FAR),
        Some(landing)
    );
    (g, cid, gun, screen, landing)
}

#[test]
fn staging_gun_crosses_a_friendly_column_to_an_open_ring_tile() {
    let (mut g, cid, gun, screen, landing) = crowded_approach();
    let screen_pos = g.units[&screen].pos;
    let city = CityView::of(&g, cid).unwrap();
    let plan = plan_against(&g, cid);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    assert!(ai.siege_stage_step(&mut g, 0, gun, &city, &plan));
    assert_eq!(
        g.units[&gun].pos, landing,
        "staging must not spend the turn parked behind its own screen"
    );
    assert_eq!(g.units[&screen].pos, screen_pos);
    assert!(g.wdist(g.units[&gun].pos, city.pos) > CITY_STRIKE_RANGE);
}

#[test]
fn staging_gun_without_enough_movement_cannot_stop_on_its_screen() {
    let (mut g, cid, gun, screen, _) = crowded_approach();
    g.units.get_mut(&gun).unwrap().moves_left = 1.0;
    let start = g.units[&gun].pos;
    let city = CityView::of(&g, cid).unwrap();
    let plan = plan_against(&g, cid);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.siege_stage_step(&mut g, 0, gun, &city, &plan);
    assert_eq!(g.units[&gun].pos, start);
    assert_ne!(g.units[&gun].pos, g.units[&screen].pos);
}
