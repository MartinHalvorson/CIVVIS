use super::tests::walled_city;
use super::*;
use crate::ai::{ForcePosture, GrandStrategy};

fn town() -> (Game, u32) {
    let (mut g, cid) = walled_city();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.current = 1;
    (g, cid)
}

fn posts(g: &Game, cid: u32, members: &[u32], heals: bool) -> BTreeMap<u32, Pos> {
    anvil_orders_for(g, 1, &CityView::of(g, cid).unwrap(), members, &[], heals)
}

#[test]
fn a_nonmember_melee_garrison_keeps_the_city_post() {
    let (mut g, cid) = town();
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let city = g.cities[&cid].pos;
    let warrior = g.spawn_unit("warrior", 1, city);
    let archer = g.spawn_unit("archer", 1, city);
    let assigned = posts(&g, cid, &[archer], true);
    assert_ne!(assigned.get(&archer), Some(&city));
    assert!(
        assigned.contains_key(&archer),
        "an open nearby post is available"
    );
    assert!(
        !assigned.contains_key(&warrior),
        "do not recruit another unit into the formation"
    );
    assert_eq!(g.units[&warrior].pos, city);
}

#[test]
fn a_nonmember_ranged_garrison_is_reserved_even_behind_walls() {
    let (mut g, cid) = town();
    let city = g.cities[&cid].pos;
    let artillery = g.spawn_unit("artillery", 1, city);
    let archer = g.spawn_unit("archer", 1, city);
    let assigned = posts(&g, cid, &[archer], false);
    assert_ne!(assigned.get(&archer), Some(&city));
    assert_eq!(g.units[&artillery].pos, city);
}

#[test]
fn a_civilian_link_does_not_make_its_combat_guard_an_open_post() {
    let (mut g, cid) = town();
    let city = g.cities[&cid].pos;
    let warrior = g.spawn_unit("warrior", 1, city);
    let settler = g.spawn_unit("settler", 1, city);
    g.units.get_mut(&warrior).unwrap().linked_to = Some(settler);
    g.units.get_mut(&settler).unwrap().linked_to = Some(warrior);
    let archer = g.spawn_unit("archer", 1, city);
    assert_eq!(arm_of(&g, warrior), Arm::Other);
    let assigned = posts(&g, cid, &[archer, warrior], true);
    assert_ne!(assigned.get(&archer), Some(&city));
    assert!(!assigned.contains_key(&warrior));
}

#[test]
fn wounded_members_do_not_claim_another_garrisons_city_to_heal() {
    let (mut g, cid) = town();
    let city = g.cities[&cid].pos;
    g.spawn_unit("warrior", 1, city);
    let archer = g.spawn_unit("archer", 1, city);
    g.units.get_mut(&archer).unwrap().hp = 10;
    assert_ne!(posts(&g, cid, &[archer], true).get(&archer), Some(&city));
}

#[test]
fn the_current_tile_fast_path_does_not_keep_a_shared_ring_post() {
    let (mut g, cid) = town();
    let city = g.cities[&cid].pos;
    g.spawn_unit("warrior", 1, city);
    let ring = g
        .wring(city, 1)
        .into_iter()
        .find(|p| g.map.get(*p).is_some())
        .unwrap();
    g.spawn_unit("warrior", 1, ring);
    let archer = g.spawn_unit("archer", 1, ring);
    let assigned = posts(&g, cid, &[archer], false);
    assert_ne!(assigned.get(&archer), Some(&ring));
    assert_ne!(assigned.get(&archer), Some(&city));
}

#[test]
fn every_assigned_post_avoids_nonmember_land_combat_units() {
    let (mut g, cid) = town();
    let city = g.cities[&cid].pos;
    let mut held = BTreeSet::new();
    for tile in g.wdisk(city, 1) {
        if g.map.get(tile).is_some() {
            g.spawn_unit("warrior", 1, tile);
            held.insert(tile);
        }
    }
    let a = g.spawn_unit("archer", 1, city);
    let b = g.spawn_unit("slinger", 1, city);
    let assigned = posts(&g, cid, &[a, b], false);
    assert_eq!(assigned.len(), 2);
    assert!(assigned.values().all(|p| !held.contains(p)));
    assert_eq!(assigned.values().copied().collect::<BTreeSet<_>>().len(), 2);
}

#[test]
fn a_completely_held_perimeter_does_not_invent_a_post() {
    let (mut g, cid) = town();
    let city = g.cities[&cid].pos;
    for tile in g.wdisk(city, 2) {
        if g.map.get(tile).is_some() {
            g.spawn_unit("warrior", 1, tile);
        }
    }
    let archer = g.spawn_unit("archer", 1, city);
    assert!(posts(&g, cid, &[archer], false).is_empty());
}

#[test]
fn formation_members_still_share_the_post_assignment_and_rotation() {
    let (mut g, cid) = town();
    let city = g.cities[&cid].pos;
    let warrior = g.spawn_unit("warrior", 1, city);
    let archer = g.spawn_unit("archer", 1, city);
    let assigned = posts(&g, cid, &[warrior, archer], false);
    assert_eq!(assigned.get(&archer), Some(&city));
    assert_ne!(assigned.get(&warrior), Some(&city));
}

#[test]
fn compatible_civilian_support_and_naval_occupants_do_not_reserve_land_posts() {
    for kind in ["settler", "battering_ram", "galley"] {
        let (mut g, cid) = town();
        let city = g.cities[&cid].pos;
        g.spawn_unit(kind, 1, city);
        let archer = g.spawn_unit("archer", 1, city);
        assert_eq!(
            posts(&g, cid, &[archer], false).get(&archer),
            Some(&city),
            "{kind}"
        );
    }
}

#[test]
fn the_doctrine_moves_the_shooter_without_moving_the_outside_garrison() {
    let (mut g, cid) = town();
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let city = g.cities[&cid].pos;
    let warrior = g.spawn_unit("warrior", 1, city);
    let archer = g.spawn_unit("archer", 1, city);
    let group = ForceGroup {
        id: 1,
        domain: ForceDomain::Land,
        units: vec![archer],
        anchor: city,
        objective: city,
        focus_target: None,
        posture: ForcePosture::Hold,
        readiness: 1.0,
        local_strength_ratio: 1.0,
    };
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: Some(0),
        target_city: None,
        threatened_city: Some(cid),
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.enable_anvil();
    ai.enable_anvil_garrisons_its_melee();
    ai.force_groups = vec![group.clone()];
    assert_eq!(ai.anvil_step(&mut g, 1, archer, &plan, &group), Some(true));
    assert_ne!(
        g.units[&archer].pos, city,
        "assignment must become an actual move"
    );
    assert_eq!(
        g.units[&warrior].pos, city,
        "the outside garrison remains untouched"
    );
}
