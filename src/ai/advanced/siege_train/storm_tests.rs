//! `melee-storms-an-open-city`: against a city with no standing walls the
//! assault opens when the force's blows past the city's heal take it within
//! `STORM_TURNS`, and the reserved taker joins it. Live King
//! civvis-20261005T074521Z (game 110): Tarsus stood without walls from turn
//! 67 to 73, "damage ready" in about two turns, while its only melee beside
//! it was the reserved taker; it never struck, and Tarsus built walls at 74.

use super::super::VictoryTarget;
use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;

/// The doctrine's turn for every unit of `pid`, looped while it acts.
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

fn open_city(hp: i32) -> (Game, u32) {
    let (mut g, cid) = walled_city();
    let city = g.cities.get_mut(&cid).unwrap();
    city.wall_hp = 0;
    city.hp = hp;
    (g, cid)
}

fn assaulting(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_siege_train();
    ai.enable_breach_assault();
    if gene {
        ai.enable_melee_storms_an_open_city();
    }
    ai
}

#[test]
fn an_open_city_the_blows_take_in_three_turns_opens_the_assault_under_the_gene() {
    let (g, cid) = open_city(170);
    let open = CityView::of(&g, cid).unwrap();
    // 80 a turn: two turns' 160 is short of 170 + 20, but 170 / (80 - 20)
    // is under three turns.
    assert!(!assaulting(false).assault_pays(80.0, &open));
    assert!(assaulting(true).assault_pays(80.0, &open));
    // Nothing past the heal: never.
    assert!(!assaulting(true).assault_pays(20.0, &open));
    // Standing walls keep the two-turn rule.
    let (mut walled, wid) = walled_city();
    walled.cities.get_mut(&wid).unwrap().hp = 170;
    let shut = CityView::of(&walled, wid).unwrap();
    assert!(shut.wall_hp > 0);
    assert!(!assaulting(true).assault_pays(80.0, &shut));
    // Where the two-turn rule already opens it, the gene changes nothing.
    assert!(assaulting(false).assault_pays(120.0, &open));
    assert!(assaulting(true).assault_pays(120.0, &open));
}

#[test]
fn the_reserved_taker_joins_the_assault_on_an_open_city_under_the_gene() {
    let mut outcome = Vec::new();
    for gene in [false, true] {
        let (mut g, cid) = open_city(60);
        let ring = ring_of(&g, cid);
        let taker = g.spawn_unit("warrior", 0, ring[0]);
        for pos in at_distance(&g, cid, 2).into_iter().take(2) {
            g.spawn_unit("archer", 0, pos);
        }
        let mut ai = assaulting(gene);
        let plan = plan_against(&g, cid);
        play(&mut ai, &mut g, 0, &plan);
        let city = &g.cities[&cid];
        let taker_hp = g.units.get(&taker).map(|u| u.hp);
        outcome.push((city.owner, city.hp, taker_hp));
    }
    let (off, on) = (outcome[0], outcome[1]);
    assert_eq!(
        off.2,
        Some(100),
        "without the gene the taker waits: {off:?}"
    );
    assert!(
        on.0 == 0 || on.1 < off.1,
        "the taker's blow lands on top of the archers': off {off:?}, on {on:?}"
    );
}

#[test]
fn a_taker_that_would_fall_under_the_floor_keeps_out() {
    let (mut g, cid) = open_city(60);
    let ring = ring_of(&g, cid);
    let taker = g.spawn_unit("warrior", 0, ring[0]);
    g.units.get_mut(&taker).unwrap().hp = ASSAULT_MIN_HP;
    for pos in at_distance(&g, cid, 2).into_iter().take(2) {
        g.spawn_unit("archer", 0, pos);
    }
    let mut ai = assaulting(true);
    let plan = plan_against(&g, cid);
    // Read the reply a blow would draw; the floor holds the taker back when
    // it would end under STORM_TAKER_SURVIVOR_HP.
    let mut after = g.speculative_clone();
    after
        .apply(
            0,
            &Action::Attack {
                unit: taker,
                target: g.cities[&cid].pos,
            },
        )
        .unwrap();
    let left = after.units.get(&taker).map_or(0, |u| u.hp);
    let captured = after.cities[&cid].owner == 0;
    play(&mut ai, &mut g, 0, &plan);
    if !captured && left < STORM_TAKER_SURVIVOR_HP {
        assert_eq!(g.units[&taker].hp, ASSAULT_MIN_HP, "the taker kept out");
    }
}
