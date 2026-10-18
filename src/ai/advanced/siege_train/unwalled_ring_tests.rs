use super::tests::walled_city;
use super::*;

/// `walled_city` with its walls down, on open grassland, at war, and our
/// Swordsman three tiles west of it.
fn unwalled_city_and_a_swordsman() -> (Game, u32, u32) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != city {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let start = (city.0 - 3, city.1);
    assert_eq!(g.wdist(start, city), 3, "fixture: three tiles out");
    let sword = g.spawn_unit("swordsman", 0, start);
    (g, cid, sword)
}

#[test]
fn a_melee_member_steps_onto_the_open_ring_only_under_the_gene() {
    let (mut g, cid, sword) = unwalled_city_and_a_swordsman();
    let city = CityView::of(&g, cid).unwrap();
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    assert_eq!(ai.unwalled_ring_step(&mut g, 0, sword, &city), None, "off");

    ai.enable_unwalled_ring_steps_in();
    assert_eq!(ai.unwalled_ring_step(&mut g, 0, sword, &city), Some(true));
    assert_eq!(g.wdist(g.units[&sword].pos, city.pos), 1, "on the ring");
}

#[test]
fn standing_walls_keep_the_member_off_the_ring() {
    let (mut g, cid, sword) = unwalled_city_and_a_swordsman();
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    let city = CityView::of(&g, cid).unwrap();
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_unwalled_ring_steps_in();
    assert_eq!(ai.unwalled_ring_step(&mut g, 0, sword, &city), None);
    assert_eq!(g.wdist(g.units[&sword].pos, city.pos), 3);
}
