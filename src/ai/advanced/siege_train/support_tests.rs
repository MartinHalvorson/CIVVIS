use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;
use crate::game::Item;

#[test]
fn siege_support_capture_projection_obeys_attacker_and_wall_immunities() {
    for (attacker, walls, steel, bypasses) in [
        ("warrior", vec!["walls"], false, true),
        ("spearman", vec!["walls", "medieval_walls"], false, true),
        ("horseman", vec!["walls"], false, false),
        (
            "warrior",
            vec!["walls", "medieval_walls", "renaissance_walls"],
            false,
            false,
        ),
        (
            "warrior",
            vec!["walls", "medieval_walls", "tsikhe"],
            false,
            false,
        ),
        ("warrior", vec!["walls"], true, false),
    ] {
        let (mut g, cid) = walled_city();
        g.cities.get_mut(&cid).unwrap().buildings =
            walls.into_iter().map(crate::name::Name::new).collect();
        g.cities.get_mut(&cid).unwrap().wall_hp = 400;
        if steel {
            g.players[1].techs.insert(crate::name!("steel"));
        }
        let ring = ring_of(&g, cid);
        let uid = g.spawn_unit(attacker, 0, ring[0]);
        g.spawn_unit("siege_tower", 0, ring[0]);
        let projection = taker_blow(&g, 0, uid, cid);
        if bypasses {
            assert!(projection > 1.0, "{attacker}");
        } else {
            assert_eq!(projection, 1.0, "{attacker}");
        }
        let before = g.cities[&cid].hp;
        let target = g.cities[&cid].pos;
        g.apply(0, &Action::Attack { unit: uid, target }).unwrap();
        let damage = before - g.cities[&cid].hp;
        if bypasses {
            assert!(damage > 1, "engine must agree with effective tower");
        } else {
            assert_eq!(damage, 1, "immune target must retain walls");
        }
    }
}

#[test]
fn siege_support_does_not_send_cavalry_or_obsolete_rams_into_healthy_walls() {
    for (attacker, support, medieval) in [
        ("horseman", "siege_tower", false),
        ("warrior", "battering_ram", true),
    ] {
        let (mut g, cid) = walled_city();
        if medieval {
            g.cities
                .get_mut(&cid)
                .unwrap()
                .buildings
                .push(crate::name!("medieval_walls"));
            g.cities.get_mut(&cid).unwrap().wall_hp = 200;
        }
        let ring = ring_of(&g, cid);
        let uid = g.spawn_unit(attacker, 0, ring[0]);
        g.spawn_unit(support, 0, ring[0]);
        let city = CityView::of(&g, cid).unwrap();
        let plan = plan_against(&g, cid);
        let hp = g.units[&uid].hp;
        AdvancedAi::new().siege_melee_step(&mut g, 0, uid, &city, &plan);
        assert_eq!(g.units[&uid].hp, hp);
        assert_eq!(g.cities[&cid].wall_hp, city.wall_hp);
    }
}

fn production_fixture() -> (Game, u32, u32, StrategicPlan) {
    let mut g = Game::new_full(2, 24, 16, 71_008, 160, 0, false);
    for pid in 0..2 {
        g.current = pid;
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
    }
    g.current = 0;
    let home = g.player_city_ids(0)[0];
    let target = g.player_city_ids(1)[0];
    let pos = g.cities[&home].pos;
    g.spawn_test_unit("warrior", 0, pos);
    g.spawn_test_unit("warrior", 0, pos);
    g.cities.get_mut(&home).unwrap().queue.clear();
    g.cities.get_mut(&target).unwrap().buildings =
        vec![crate::name!("walls"), crate::name!("medieval_walls")];
    g.cities.get_mut(&target).unwrap().wall_hp = 200;
    g.at_war.insert((0, 1));
    g.players[0]
        .techs
        .insert(g.rules.units["siege_tower"].tech.unwrap());
    let plan = plan_against(&g, target);
    (g, home, target, plan)
}

#[test]
fn siege_support_production_replaces_obsolete_ram_once() {
    let (mut g, home, _, plan) = production_fixture();
    let pos = g.cities[&home].pos;
    g.spawn_test_unit("battering_ram", 0, pos);
    let mut ai = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    ai.base.book_pos = 4;
    let counts = ai.counts(&g, 0);
    assert!(ai.support_unit_value(&g, 0, home, "siege_tower", &plan, &counts) > 0.0);
    ai.advanced_support_production(&mut g, 0, &plan);
    assert!(
        matches!(g.cities[&home].queue.first(), Some(Item::Unit { unit }) if unit == "siege_tower")
    );
    let reservation_counts = ai.counts_without_city_queue(&g, 0, home);
    assert!(
        ai.support_unit_value(&g, 0, home, "siege_tower", &plan, &reservation_counts) > 0.0,
        "a queued tower must not veto its own production commitment"
    );
    let counts = ai.counts(&g, 0);
    assert!(ai.support_unit_value(&g, 0, home, "siege_tower", &plan, &counts) < 0.0);
}

#[test]
fn siege_support_production_requires_live_walls_and_eligible_infantry() {
    let (mut g, home, target, plan) = production_fixture();
    let ai = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    let pos = g.cities[&home].pos;
    for _ in 0..4 {
        g.spawn_test_unit("horseman", 0, pos);
    }
    let value =
        |g: &Game| ai.support_unit_value(g, 0, home, "siege_tower", &plan, &ai.counts(g, 0));
    assert!(value(&g) < 0.0, "mounted-only armies cannot use a tower");
    g.cities.get_mut(&home).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("spearman"),
    }];
    assert!(
        value(&g) > 0.0,
        "queued eligible infantry opens the support requirement"
    );
    g.cities.get_mut(&target).unwrap().wall_hp = 0;
    assert!(value(&g) < 0.0, "breached walls need no replacement tower");
    g.cities.get_mut(&target).unwrap().wall_hp = 200;
    g.players[1].techs.insert(crate::name!("steel"));
    assert!(value(&g) < 0.0, "urban defenses require artillery");
}

#[test]
fn siege_support_moves_to_eligible_infantry_instead_of_colocated_cavalry() {
    let (mut g, cid) = walled_city();
    // Compare escorts on a safe staging ring; entering city fire is a
    // separate decision owned by the movement-risk planner.
    let ring = at_distance(&g, cid, 4);
    let first = ring[0];
    let second = *ring.iter().find(|pos| g.wdist(first, **pos) == 1).unwrap();
    let infantry = g.spawn_unit("spearman", 0, first);
    g.spawn_unit("horseman", 0, second);
    let ram = g.spawn_unit("battering_ram", 0, second);
    let before = g.wdist(g.units[&ram].pos, g.units[&infantry].pos);
    assert!(AdvancedAi::new().base.siege_support_step(&mut g, 0, ram));
    assert!(g.wdist(g.units[&ram].pos, g.units[&infantry].pos) < before);
}

#[test]
fn siege_support_movement_and_basic_production_reject_obsolete_equipment() {
    let (mut g, home, target, _) = production_fixture();
    let pos = g.cities[&home].pos;
    let ram = g.spawn_test_unit("battering_ram", 0, pos);
    let ai = AdvancedAi::new();
    assert!(!ai.base.siege_support_step(&mut g, 0, ram));
    assert_eq!(
        ai.base.siege_support_unit(&g, 0, home).as_deref(),
        Some("siege_tower")
    );
    g.cities
        .get_mut(&target)
        .unwrap()
        .buildings
        .push(crate::name!("tsikhe"));
    assert_eq!(ai.base.siege_support_unit(&g, 0, home), None);
    g.cities.get_mut(&target).unwrap().buildings.pop();
    g.players[1].techs.insert(crate::name!("steel"));
    assert_eq!(ai.base.siege_support_unit(&g, 0, home), None);
}

#[test]
fn siege_support_allows_effective_infantry_breaches() {
    for support in ["battering_ram", "siege_tower"] {
        let (mut g, cid) = walled_city();
        let pos = ring_of(&g, cid)[0];
        let uid = g.spawn_unit("infantry", 0, pos);
        g.spawn_unit(support, 0, pos);
        let city = CityView::of(&g, cid).unwrap();
        let plan = plan_against(&g, cid);
        AdvancedAi::new().siege_melee_step(&mut g, 0, uid, &city, &plan);
        if support == "battering_ram" {
            assert!(g.cities[&cid].wall_hp < city.wall_hp);
        } else {
            assert!(g.cities[&cid].hp < city.hp - 1);
        }
    }
}
