use super::super::ForcePosture;
use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;

fn group(g: &Game, uid: u32, cid: u32) -> ForceGroup {
    ForceGroup {
        id: uid,
        domain: ForceDomain::Land,
        units: vec![uid],
        anchor: g.units[&uid].pos,
        objective: g.cities[&cid].pos,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    }
}

#[test]
fn a_siege_does_not_reserve_another_citys_capture_unit() {
    let (mut g, first) = walled_city();
    let second_pos = at_distance(&g, first, 3)[0];
    g.found_city_for(1, second_pos, None);
    let second = g.city_at(second_pos).unwrap();
    let warrior_pos = ring_of(&g, second)
        .into_iter()
        .filter(|p| g.wdist(*p, g.cities[&first].pos) <= 3)
        .find(|p| g.city_at(*p).is_none() && !g.rules.is_water(g.map.get(*p).unwrap()))
        .unwrap();
    let warrior = g.spawn_unit("warrior", 0, warrior_pos);
    let gun_pos = at_distance(&g, first, 2)
        .into_iter()
        .find(|p| *p != warrior_pos && *p != second_pos)
        .unwrap();
    let gun = g.spawn_unit("catapult", 0, gun_pos);
    for cid in [first, second] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.hp = 1;
        city.wall_hp = 0;
    }
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.force_groups = vec![group(&g, gun, first), group(&g, warrior, second)];
    let plan = plan_against(&g, first);
    let _ = ai.siege_doctrine_step(&mut g, 0, gun, &plan);
    assert_ne!(
        ai.sieges[&first].taker,
        Some(warrior),
        "a nearby unit assigned to the other city is not this siege's finisher"
    );
    assert!(!ai.sieges[&first].posts.contains_key(&warrior));
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    assert_eq!(
        g.cities[&second].owner, 0,
        "the warrior completes its assigned capture"
    );
}

#[test]
fn two_groups_assigned_to_one_city_share_its_finisher() {
    let (mut g, cid) = walled_city();
    let warrior = g.spawn_unit("warrior", 0, ring_of(&g, cid)[0]);
    let gun = g.spawn_unit("catapult", 0, at_distance(&g, cid, 2)[0]);
    g.cities.get_mut(&cid).unwrap().hp = 1;
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.force_groups = vec![group(&g, gun, cid), group(&g, warrior, cid)];
    let plan = plan_against(&g, cid);
    let _ = ai.siege_doctrine_step(&mut g, 0, gun, &plan);
    assert_eq!(ai.sieges[&cid].taker, Some(warrior));
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    assert_eq!(g.cities[&cid].owner, 0);
}

fn taker_with_builder(distance: i32) -> (Game, AdvancedAi, StrategicPlan, u32, u32, u32) {
    let (mut g, cid) = walled_city();
    let origin = ring_of(&g, cid)[0];
    let warrior = g.spawn_unit("warrior", 0, origin);
    let gun = g.spawn_unit("catapult", 0, at_distance(&g, cid, 2)[0]);
    g.cities.get_mut(&cid).unwrap().hp = 1;
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.base.barbarian_settler_capture = true;
    ai.force_groups = vec![group(&g, gun, cid), group(&g, warrior, cid)];
    let plan = plan_against(&g, cid);
    let _ = ai.siege_doctrine_step(&mut g, 0, gun, &plan);
    assert!(ai.unit_is_reserved(warrior));
    let prize = g
        .wring(origin, distance)
        .into_iter()
        .find(|pos| {
            g.city_at(*pos).is_none()
                && g.unit_ids_at(*pos).is_empty()
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                && g.wdist(*pos, g.cities[&cid].pos) > 1
        })
        .unwrap();
    let builder = g.spawn_unit("builder", 1, prize);
    let mut pickup = g.clone();
    let can_pick_up = if distance == 1 {
        ai.base
            .capture_adjacent_civilian(&mut pickup, 0, warrior, false)
    } else {
        ai.base
            .pursue_capturable_civilian(&mut pickup, 0, warrior, false)
    };
    assert!(can_pick_up, "the civilian competes with the siege decision");
    (g, ai, plan, cid, warrior, builder)
}

#[test]
fn reserved_city_taker_finishes_before_an_adjacent_builder_pickup() {
    let (mut g, mut ai, plan, cid, warrior, builder) = taker_with_builder(1);
    assert!(ai.advanced_military_step(&mut g, 0, warrior, &plan));
    assert_eq!(g.cities[&cid].owner, 0);
    assert_eq!(g.units[&builder].owner, 1);
}

#[test]
fn reserved_city_taker_finishes_before_a_distant_builder_pursuit() {
    let (mut g, mut ai, plan, cid, warrior, builder) = taker_with_builder(2);
    assert!(ai.advanced_military_step(&mut g, 0, warrior, &plan));
    assert_eq!(g.cities[&cid].owner, 0);
    assert_eq!(g.units[&builder].owner, 1);
}

#[test]
fn obsolete_siege_reservation_still_allows_a_civilian_pickup() {
    let (mut g, mut ai, plan, cid, warrior, builder) = taker_with_builder(1);
    g.cities.get_mut(&cid).unwrap().owner = 0;
    assert!(ai.advanced_military_step(&mut g, 0, warrior, &plan));
    assert_eq!(g.units[&builder].owner, 0);
}

#[test]
fn an_unreserved_soldier_can_still_pick_up_a_civilian() {
    let (mut g, mut ai, plan, cid, warrior, builder) = taker_with_builder(1);
    ai.reserved_units.clear();
    assert!(ai.advanced_military_step(&mut g, 0, warrior, &plan));
    assert_eq!(g.units[&builder].owner, 0);
    assert_eq!(g.cities[&cid].owner, 1);
}
