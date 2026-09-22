use super::*;

fn pursuit() -> (Game, BasicAi, u32, u32) {
    let mut g = Game::new_full(2, 24, 18, 91_773, 120, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.owner_city = None;
        tile.improvement = None;
        tile.district = None;
    }
    let city = g.found_city_for(1, (8, 8), None);
    g.cities.get_mut(&city).unwrap().wall_hp = 100;
    g.at_war.insert((0, 1));
    let soldier = g.spawn_test_unit("tank", 0, (4, 8));
    g.units.get_mut(&soldier).unwrap().moves_left = 6.0;
    g.spawn_test_unit("builder", 1, (7, 8));
    let mut ai = BasicAi::new();
    ai.barbarian_settler_capture = true;
    (g, ai, soldier, city)
}

#[test]
fn incidental_pursuit_declines_a_civilian_under_city_fire() {
    let (mut g, ai, soldier, _) = pursuit();
    let origin = g.units[&soldier].pos;
    assert!(!ai.pursue_capturable_civilian(&mut g, 0, soldier, false));
    assert_eq!(g.units[&soldier].pos, origin);
}

#[test]
fn unwalled_city_does_not_prevent_civilian_pursuit() {
    let (mut g, ai, soldier, city) = pursuit();
    g.cities.get_mut(&city).unwrap().wall_hp = 0;
    assert!(ai.pursue_capturable_civilian(&mut g, 0, soldier, false));
}

#[test]
fn protected_prize_does_not_hide_an_unprotected_alternative() {
    let (mut g, ai, soldier, _) = pursuit();
    let safe = (4, 5);
    g.spawn_test_unit("builder", 1, safe);
    assert!(ai.pursue_capturable_civilian(&mut g, 0, soldier, false));
    assert!(g.wdist(g.units[&soldier].pos, safe) < 3);
}

#[test]
fn active_encampment_protects_a_civilian_even_without_city_walls() {
    let (mut g, ai, soldier, city) = pursuit();
    let enc = (7, 9);
    g.map.tiles.get_mut(&enc).unwrap().district = Some(crate::name!("encampment"));
    g.map.tiles.get_mut(&enc).unwrap().owner_city = Some(city);
    let c = g.cities.get_mut(&city).unwrap();
    c.wall_hp = 0;
    c.districts.insert(crate::name!("encampment"), enc);
    c.encampment_hp = 100;
    c.encampment_wall_hp = 100;
    assert!(!ai.pursue_capturable_civilian(&mut g, 0, soldier, false));
    g.cities.get_mut(&city).unwrap().encampment_pillaged = true;
    assert!(ai.pursue_capturable_civilian(&mut g, 0, soldier, false));
}
