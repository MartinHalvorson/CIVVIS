//! `capture-holds-the-ring`: see `capture_hold`.

use std::sync::Arc;

use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;
use crate::ai::advanced::ForcePosture;

/// `walled_city` on open grassland, at war, its walls down and its health
/// at `hp`.
fn open_city(hp: i32) -> (Game, u32) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != city {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
    g.at_war.insert((0, 1));
    let city = g.cities.get_mut(&cid).unwrap();
    city.wall_hp = 0;
    city.hp = hp;
    (g, cid)
}

fn holding(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    if gene {
        ai.enable_capture_holds_the_ring();
    }
    ai
}

/// A hostile Swordsman two tiles from the city, beside the ring tile `near`.
fn retaker(g: &mut Game, cid: u32, near: Pos) -> u32 {
    let at = at_distance(g, cid, 2)
        .into_iter()
        .find(|pos| g.wdist(*pos, near) == 1)
        .expect("a tile two out beside the ring tile");
    let enemy = g.spawn_unit("swordsman", 1, at);
    assert!(g.unit_visible_to(enemy, 0), "fixture: our taker sees it");
    enemy
}

fn land_group(g: &Game, cid: u32, units: &[u32]) -> ForceGroup {
    let at = g.cities[&cid].pos;
    ForceGroup {
        id: units[0],
        domain: ForceDomain::Land,
        units: units.to_vec(),
        anchor: at,
        objective: at,
        focus_target: Some(at),
        posture: ForcePosture::Muster,
        readiness: 0.0,
        local_strength_ratio: 1.0,
    }
}

#[test]
fn a_capture_a_hostile_could_retake_waits_for_two_escorts() {
    let (mut g, cid) = open_city(1);
    let ring = ring_of(&g, cid);
    let taker = g.spawn_unit("warrior", 0, ring[0]);
    let enemy = retaker(&mut g, cid, ring[0]);
    assert!(
        holding(false).capture_holdable(&g, 0, cid, taker),
        "gene off"
    );
    let ai = holding(true);
    assert!(!ai.capture_holdable(&g, 0, cid, taker));
    // One escort beside the city is not enough; two are.
    let free: Vec<Pos> = ring
        .iter()
        .copied()
        .filter(|pos| g.unit_ids_at(*pos).is_empty())
        .collect();
    g.spawn_unit("warrior", 0, free[0]);
    assert!(!ai.capture_holdable(&g, 0, cid, taker));
    g.spawn_unit("warrior", 0, free[1]);
    assert!(ai.capture_holdable(&g, 0, cid, taker));
    // No retaker in reach: always.
    let (mut lone, lid) = open_city(1);
    let lone_taker = lone.spawn_unit("warrior", 0, ring_of(&lone, lid)[0]);
    assert!(ai.capture_holdable(&lone, 0, lid, lone_taker));
    let _ = enemy;
}

#[test]
fn the_taker_holds_off_a_capture_it_cannot_keep_only_under_the_gene() {
    for gene in [false, true] {
        let (mut g, cid) = open_city(1);
        let ring = ring_of(&g, cid);
        let taker = g.spawn_unit("warrior", 0, ring[0]);
        retaker(&mut g, cid, ring[0]);
        let group = land_group(&g, cid, &[taker]);
        let plan = plan_against(&g, cid);
        let mut ai = holding(gene);
        ai.siege_train_step(&mut g, 0, taker, cid, &plan, &group);
        let owner = g.cities[&cid].owner;
        if gene {
            assert_eq!(owner, 1, "the capture waits for escorts");
        } else {
            assert_eq!(owner, 0, "without the gene the taker walks in");
        }
    }
}

/// The city taken by us this turn, its loyalty falling at `rate` from `loyalty`.
fn captured(g: &mut Game, cid: u32, loyalty: f64, rate: f64) {
    let city = g.cities.get_mut(&cid).unwrap();
    city.owner = 0;
    city.occupied_from = Some(1);
    city.loyalty = loyalty;
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(cid, rate);
}

#[test]
fn the_army_holds_the_ring_of_a_city_it_just_took() {
    for gene in [false, true] {
        let (mut g, cid) = open_city(100);
        captured(&mut g, cid, 80.0, 2.0);
        let start = at_distance(&g, cid, 3)[0];
        let body = g.spawn_unit("swordsman", 0, start);
        let mut ai = holding(gene);
        ai.note_capture_holds(&g, 0);
        let acted = ai.capture_ring_step(&mut g, 0, body);
        if gene {
            assert_eq!(acted, Some(true));
            assert_eq!(
                g.wdist(g.units[&body].pos, g.cities[&cid].pos),
                1,
                "on the ring"
            );
        } else {
            assert_eq!(acted, None);
            assert_eq!(g.units[&body].pos, start);
        }
    }
}

#[test]
fn a_shooter_covering_the_ring_strikes_the_nearest_hostile() {
    let (mut g, cid) = open_city(100);
    captured(&mut g, cid, 80.0, 2.0);
    let ring = ring_of(&g, cid);
    let archer = g.spawn_unit("archer", 0, ring[0]);
    let enemy = retaker(&mut g, cid, ring[0]);
    let mut ai = holding(true);
    ai.note_capture_holds(&g, 0);
    assert_eq!(ai.capture_ring_step(&mut g, 0, archer), Some(true));
    assert!(
        g.units.get(&enemy).is_none_or(|unit| unit.hp < 100),
        "the archer fired"
    );
}

#[test]
fn the_ring_holds_one_turn_past_the_capture() {
    let (mut g, cid) = open_city(100);
    captured(&mut g, cid, 80.0, 2.0);
    let body = g.spawn_unit("swordsman", 0, at_distance(&g, cid, 3)[0]);
    let mut ai = holding(true);
    ai.note_capture_holds(&g, 0);
    g.turn += capture_hold::CAPTURE_HOLD_TURNS + 1;
    ai.note_capture_holds(&g, 0);
    assert_eq!(
        ai.capture_ring_step(&mut g, 0, body),
        None,
        "the hold has lapsed"
    );
}

#[test]
fn a_capture_whose_loyalty_runs_out_sends_the_force_on_its_pressure() {
    let (mut g, cid) = open_city(100);
    captured(&mut g, cid, 20.0, -10.0);
    assert!(holding(true).capture_runway(&g, cid) < capture_hold::CAPTURE_RUNWAY_TURNS);
    let site = at_distance(&g, cid, 5)[0];
    let pressure = g.found_city_for(1, site, None);
    g.cities.get_mut(&pressure).unwrap().pop = 12;
    let body = g.spawn_unit("swordsman", 0, at_distance(&g, cid, 3)[0]);
    let anchor = g.units[&body].pos;
    let mut ai = holding(true);
    ai.note_capture_holds(&g, 0);
    assert_eq!(
        ai.capture_ring_step(&mut g, 0, body),
        None,
        "no ring to hold"
    );
    assert_eq!(ai.loyalty_prey_for(&g, 0, anchor), Some(pressure));
    let group = land_group(&g, cid, &[body]);
    let plan = plan_against(&g, cid);
    assert_eq!(ai.siege_city_of(&g, 0, &plan, &group), Some(pressure));
    // With the gene off nothing changes.
    let mut off = holding(false);
    off.note_capture_holds(&g, 0);
    assert_eq!(off.loyalty_prey_for(&g, 0, anchor), None);
}

#[test]
fn a_dying_open_city_is_never_staged_for() {
    for gene in [false, true] {
        let (mut g, cid) = open_city(40);
        let far = at_distance(&g, cid, STAGING_FAR + 2);
        let bodies: Vec<u32> = far
            .iter()
            .take(2)
            .map(|pos| g.spawn_unit("swordsman", 0, *pos))
            .collect();
        let group = land_group(&g, cid, &bodies);
        let plan = plan_against(&g, cid);
        let mut ai = holding(gene);
        ai.assess_siege(&g, 0, cid, &plan, &group);
        let siege = &ai.sieges[&cid];
        if gene {
            assert_eq!(siege.stage, SiegeStage::Invest);
            // The bodies take ring posts and walk in (`post_step`); one that
            // can reach the ring this turn becomes the taker.
            let at = g.cities[&cid].pos;
            assert!(
                bodies.iter().any(|uid| siege
                    .posts
                    .get(uid)
                    .is_some_and(|post| g.wdist(*post, at) == 1)),
                "the nearest body is called in: {:?}",
                siege.posts
            );
        } else {
            assert_eq!(siege.stage, SiegeStage::Stage);
        }
    }
}
