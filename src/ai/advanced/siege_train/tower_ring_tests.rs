//! `tower-assault` and `ring-fires-on-the-city`.

use std::sync::Arc;

use super::super::VictoryTarget;
use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;

/// `walled_city` on open grassland, at war, behind `walls` of `buildings`.
fn city_behind(buildings: &[&str]) -> (Game, u32) {
    let (mut g, cid) = walled_city();
    let at = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != at {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
    g.at_war.insert((0, 1));
    g.turn = 30;
    let city = g.cities.get_mut(&cid).unwrap();
    city.buildings = buildings
        .iter()
        .map(|b| crate::name::Name::from(*b))
        .collect();
    Arc::make_mut(&mut g.observed_city_max_wall_hp).remove(&cid);
    let max = g.city_max_wall_hp(&g.cities[&cid]);
    g.cities.get_mut(&cid).unwrap().wall_hp = max;
    (g, cid)
}

fn group_of(g: &Game, cid: u32, units: &[u32]) -> ForceGroup {
    let at = g.cities[&cid].pos;
    ForceGroup {
        id: units[0],
        domain: ForceDomain::Land,
        units: units.to_vec(),
        anchor: at,
        objective: at,
        focus_target: Some(at),
        posture: super::super::ForcePosture::Engage,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    }
}

#[test]
fn melee_beside_a_siege_tower_storms_the_city_through_the_walls() {
    for gene in [false, true] {
        let (mut g, cid) = city_behind(&["walls", "medieval_walls"]);
        let walls = g.cities[&cid].wall_hp;
        assert_eq!(walls, 200, "fixture: Medieval Walls");
        let ring = ring_of(&g, cid);
        g.spawn_unit("siege_tower", 0, ring[0]);
        let pikes: Vec<u32> = ring[1..4]
            .iter()
            .map(|pos| g.spawn_unit("pike_and_shot", 0, *pos))
            .collect();
        let city = CityView::of(&g, cid).unwrap();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_siege_train();
        ai.enable_breach_assault();
        if gene {
            ai.enable_tower_assault();
        }
        assert_eq!(ai.tower_bypasses(&g, 0, pikes[0], &city), gene);
        let plan = plan_against(&g, cid);
        let group = group_of(&g, cid, &pikes);
        let acted = ai.breach_assault_blow(&mut g, 0, pikes[0], &city, &plan, &group);
        let after = &g.cities[&cid];
        if gene {
            assert_eq!(acted, Some(true));
            assert!(after.hp < 200, "the blow lands on the city's health");
            assert!(
                walls - after.wall_hp < 200 - after.hp,
                "most of it through the walls: walls {walls} -> {}, city 200 -> {}",
                after.wall_hp,
                after.hp
            );
        } else {
            assert_eq!(acted, None, "walls plus city are out of the volley's reach");
            assert_eq!((after.hp, after.wall_hp), (200, walls));
        }
    }
}

#[test]
fn cavalry_beside_a_tower_gets_no_bypass() {
    let (mut g, cid) = city_behind(&["walls", "medieval_walls"]);
    let ring = ring_of(&g, cid);
    g.spawn_unit("siege_tower", 0, ring[0]);
    let knight = g.spawn_unit("knight", 0, ring[1]);
    let city = CityView::of(&g, cid).unwrap();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_tower_assault();
    assert!(!ai.tower_bypasses(&g, 0, knight, &city));
}

/// An Archer two tiles from walled `city_behind`, a hostile Warrior it can
/// reach, and nothing it can kill.
fn archer_and_warrior(g: &mut Game, cid: u32) -> (u32, u32) {
    let post = at_distance(g, cid, 2)[0];
    let archer = g.spawn_unit("archer", 0, post);
    let enemy_at = g
        .nbrs(post)
        .into_iter()
        .find(|pos| g.wdist(*pos, g.cities[&cid].pos) >= 2 && g.unit_ids_at(*pos).is_empty())
        .unwrap();
    let warrior = g.spawn_unit("warrior", 1, enemy_at);
    (archer, warrior)
}

#[test]
fn a_ring_shooter_fires_on_the_city_rather_than_a_unit_under_the_gene() {
    for gene in [false, true] {
        let (mut g, cid) = city_behind(&["walls"]);
        let (archer, warrior) = archer_and_warrior(&mut g, cid);
        let before = (g.cities[&cid].hp, g.cities[&cid].wall_hp);
        let mut ai = AdvancedAi::new();
        ai.enable_siege_train();
        if gene {
            ai.enable_ring_fires_on_the_city();
        }
        let city = CityView::of(&g, cid).unwrap();
        assert!(ai.siege_shooter_step(&mut g, 0, archer, &city));
        let after = (g.cities[&cid].hp, g.cities[&cid].wall_hp);
        let hit_unit = g.units.get(&warrior).is_none_or(|unit| unit.hp < 100);
        if gene {
            assert_ne!(after, before, "the shot went to the city");
            assert!(!hit_unit);
        } else {
            assert_eq!(after, before);
            assert!(hit_unit, "the shot went to the Warrior");
        }
    }
}

#[test]
fn a_hostile_beside_a_wounded_friend_still_draws_the_shot() {
    let (mut g, cid) = city_behind(&["walls"]);
    let (archer, warrior) = archer_and_warrior(&mut g, cid);
    let beside = g
        .nbrs(g.units[&warrior].pos)
        .into_iter()
        .find(|pos| g.unit_ids_at(*pos).is_empty() && g.city_at(*pos).is_none())
        .unwrap();
    let friend = g.spawn_unit("swordsman", 0, beside);
    g.units.get_mut(&friend).unwrap().hp = 30;
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_ring_fires_on_the_city();
    assert!(ai.ring_is_threatened(&g, 0, archer));
    let city = CityView::of(&g, cid).unwrap();
    assert!(ai.siege_shooter_step(&mut g, 0, archer, &city));
    assert!(g.units.get(&warrior).is_none_or(|unit| unit.hp < 100));
}
