use super::super::GrandStrategy;
use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::doctrine::{build, position};

/// `the_storming`'s city of player 1 with every unit removed and its walls
/// knocked down: an unwalled frontier town, with our seat as player 1. Its
/// strongest unit built is a Warrior, as at the live Corinths (t15-28), so
/// the bare town's defence floor is 10.
fn unwalled_town() -> (Game, u32) {
    let mut g = build(position("the_storming").expect("known"), 3).expect("buildable");
    let seeded: Vec<u32> = (0..2).flat_map(|pid| g.player_unit_ids(pid)).collect();
    for uid in seeded {
        g.remove_unit(uid);
    }
    let cid = *g.cities.keys().next().expect("the position states a city");
    assert_eq!(g.cities[&cid].owner, 1);
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    g.players[1]
        .counters
        .insert("strongest_unit_built".to_string(), 20);
    g.current = 1;
    (g, cid)
}

fn holding(g: &Game, cid: u32) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: Some(0),
        target_city: None,
        threatened_city: Some(cid),
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    }
}

/// Land tiles exactly `distance` from the city, sorted.
fn at_distance(g: &Game, cid: u32, distance: i32) -> Vec<Pos> {
    let pos = g.cities[&cid].pos;
    let mut out: Vec<Pos> = g
        .wring(pos, distance)
        .into_iter()
        .filter(|p| {
            g.map
                .get(*p)
                .is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
        })
        .collect();
    out.sort_unstable();
    out
}

/// Every unit of `pid` through the doctrine, as the ladder loops a unit
/// while it acts (`siege_train`'s own `play`).
fn play(ai: &mut AdvancedAi, g: &mut Game, pid: usize, plan: &StrategicPlan) {
    ai.rebuild_force_groups(g, pid, plan);
    let mut ids = g.player_unit_ids(pid);
    ids.sort_unstable();
    for uid in ids {
        for _ in 0..8 {
            if !g.units.contains_key(&uid) || g.units[&uid].moves_left <= 0.0 {
                break;
            }
            if ai.force_groups_dirty {
                ai.rebuild_force_groups(g, pid, plan);
                ai.force_groups_dirty = false;
            }
            match ai.siege_doctrine_step(g, pid, uid, plan) {
                Some(true) => {}
                _ => break,
            }
        }
    }
}

fn anvil(garrisons: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_anvil();
    if garrisons {
        ai.enable_anvil_garrisons_its_melee();
    }
    ai
}

#[test]
fn anvil_garrisons_its_melee_is_opt_in() {
    opt_in_off_in_both_controllers("anvil-garrisons-its-melee", |ai| {
        ai.anvil_garrisons_its_melee
    });
}

/// 201608Z t26-27: a Warrior inside the unwalled Corinth, a Slinger two
/// tiles out, a hostile four out. The Anvil alone posts the Slinger to the
/// city and walks the Warrior to the ring; with the gene the Warrior stays.
#[test]
fn the_warrior_inside_an_unwalled_town_stays_inside() {
    for garrisons in [false, true] {
        let (mut g, cid) = unwalled_town();
        let city = g.cities[&cid].pos;
        let warrior = g.spawn_unit("warrior", 1, city);
        let slinger = g.spawn_unit("slinger", 1, at_distance(&g, cid, 2)[0]);
        g.spawn_unit("warrior", 0, at_distance(&g, cid, 4)[0]);
        let before = g.city_strength(cid);
        let mut ai = anvil(garrisons);
        let plan = holding(&g, cid);
        play(&mut ai, &mut g, 1, &plan);
        if garrisons {
            assert_eq!(g.units[&warrior].pos, city, "the Warrior holds the town");
            assert_ne!(ai.anvil_orders.get(&slinger), Some(&city));
            assert_eq!(g.city_strength(cid), before, "the town keeps its defence");
        } else {
            assert_ne!(g.units[&warrior].pos, city, "the Anvil alone walks it out");
            assert_eq!(ai.anvil_orders.get(&slinger), Some(&city));
            assert!(g.city_strength(cid) < before, "and the defence falls");
        }
    }
}

/// 200156Z t24-26: the Warrior on the ring, the Slinger posted to the city
/// still three tiles off, the city bare. With the gene the Warrior steps in
/// this turn.
#[test]
fn the_warrior_on_the_ring_steps_into_a_bare_town() {
    for garrisons in [false, true] {
        let (mut g, cid) = unwalled_town();
        let city = g.cities[&cid].pos;
        let warrior = g.spawn_unit("warrior", 1, at_distance(&g, cid, 1)[0]);
        g.spawn_unit("slinger", 1, at_distance(&g, cid, 3)[0]);
        g.spawn_unit("warrior", 0, at_distance(&g, cid, 4)[2]);
        let mut ai = anvil(garrisons);
        let plan = holding(&g, cid);
        play(&mut ai, &mut g, 1, &plan);
        assert_eq!(
            g.units[&warrior].pos == city,
            garrisons,
            "the Warrior garrisons the bare town only with the gene"
        );
    }
}

/// A Slinger in the city and a Warrior beside it: they trade places at
/// once, and the Slinger stands within two of the town, not in it.
#[test]
fn the_slinger_inside_trades_places_with_the_warrior() {
    let (mut g, cid) = unwalled_town();
    let city = g.cities[&cid].pos;
    let slinger = g.spawn_unit("slinger", 1, city);
    let warrior = g.spawn_unit("warrior", 1, at_distance(&g, cid, 1)[0]);
    g.spawn_unit("warrior", 0, at_distance(&g, cid, 4)[0]);
    let mut ai = anvil(true);
    let plan = holding(&g, cid);
    play(&mut ai, &mut g, 1, &plan);
    assert_eq!(g.units[&warrior].pos, city);
    let off = g.wdist(g.units[&slinger].pos, city);
    assert!(
        (1..=2).contains(&off),
        "the Slinger stands behind, {off} out"
    );
}

/// Walls standing: the city shoots, the Shooter keeps the post, and the
/// gene changes nothing.
#[test]
fn walls_standing_keep_the_shooter_in_the_city() {
    let (mut g, cid) = unwalled_town();
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    let city = g.cities[&cid].pos;
    let warrior = g.spawn_unit("warrior", 1, city);
    let slinger = g.spawn_unit("slinger", 1, at_distance(&g, cid, 2)[0]);
    g.spawn_unit("warrior", 0, at_distance(&g, cid, 4)[0]);
    let mut ai = anvil(true);
    let plan = holding(&g, cid);
    play(&mut ai, &mut g, 1, &plan);
    assert_eq!(ai.anvil_orders.get(&slinger), Some(&city));
    assert_ne!(ai.anvil_orders.get(&warrior), Some(&city));
}

/// The garrison holds rather than strikes: a wounded hostile beside the
/// town is left for the ring and the Slinger.
#[test]
fn the_garrison_does_not_attack_out() {
    let (mut g, cid) = unwalled_town();
    let city = g.cities[&cid].pos;
    let warrior = g.spawn_unit("warrior", 1, city);
    let prey = g.spawn_unit("warrior", 0, at_distance(&g, cid, 1)[0]);
    g.units.get_mut(&prey).unwrap().hp = 15;
    let mut ai = anvil(true);
    let plan = holding(&g, cid);
    play(&mut ai, &mut g, 1, &plan);
    assert_eq!(g.units[&warrior].pos, city);
    assert!(g.units.contains_key(&prey), "no blow out of the town");
    assert!(ai.anvil_garrison_holds(&g, warrior, city, city, 0));
    assert!(
        !ai.anvil_garrison_holds(&g, warrior, city, city, 50),
        "walls up"
    );
}

/// Only a soldier that lifts the town's defence takes the post: a Scout
/// (melee 10) does not, and neither does a Warrior once the strongest unit
/// built sets the bare town's floor at 30.
#[test]
fn only_a_garrison_that_lifts_the_defence_takes_the_post() {
    let (mut g, cid) = unwalled_town();
    let city = g.cities[&cid].pos;
    let ring = at_distance(&g, cid, 1);
    let scout = g.spawn_unit("scout", 1, ring[0]);
    assert_eq!(melee_garrison(&g, 1, city, &[scout], None), None);
    let warrior = g.spawn_unit("warrior", 1, ring[1]);
    assert_eq!(
        melee_garrison(&g, 1, city, &[scout, warrior], None),
        Some(warrior)
    );
    let slinger = g.spawn_unit("slinger", 1, city);
    assert_eq!(
        melee_garrison(&g, 1, city, &[warrior], Some(slinger)),
        Some(warrior)
    );
    assert_eq!(
        melee_garrison(&g, 1, city, &[scout], Some(warrior)),
        None,
        "a melee holder keeps the post"
    );
    g.players[1]
        .counters
        .insert("strongest_unit_built".to_string(), 40);
    assert_eq!(melee_garrison(&g, 1, city, &[warrior], None), None);
    let far = g.spawn_unit("swordsman", 1, at_distance(&g, cid, 3)[0]);
    assert_eq!(
        melee_garrison(&g, 1, city, &[far], None),
        None,
        "three tiles off cannot hold it this turn"
    );
}
