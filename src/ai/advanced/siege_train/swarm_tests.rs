//! `unwalled-city-takes-the-swarm`: against a city with no wall pool at all
//! (`walls 0/0`) every fit melee member is per-turn fire in the siege budget,
//! its reply charged to its endurance, and every fit melee member beside the
//! city swings each turn. Live 10-05/06: 176 sieges reached a city with no
//! walls and 107 never took it; 63% of those Stage/Invest rows read "damage
//! ready false with inf turns". Kish (G183, civvis-20261006T021637Z) took one
//! blow, at turn 96, and stood behind walls by 98.

use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;

/// The swarm's Kish: `walled_city` with no wall pool at all (the host's
/// `walls 0/0`), a City Center as strong as `strength` with an Archer's
/// strike, and a board that heals, so a unit under `ROTATE_HP` rotates out
/// (`siege_member_fit`).
fn unwalled_city(strength: f64) -> (Game, u32) {
    let (mut g, cid) = walled_city();
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 0);
    assert_eq!(g.city_max_wall_hp(&g.cities[&cid]), 0, "no wall pool");
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, strength);
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, 25.0);
    g.tactics.heal = true;
    (g, cid)
}

/// The deployed budget (`siege-budget-counts-what-fires`), with or without
/// the gene.
fn budgeting(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_siege_budget_counts_what_fires();
    ai.enable_siege_positive_damage_budget();
    if gene {
        ai.enable_unwalled_city_takes_the_swarm();
    }
    ai
}

/// Three Men-at-Arms three tiles out: in the budget's radius, off the ring,
/// so the ring stays open and the city heals.
fn melee_train(g: &mut Game, cid: u32, hp: i32) -> Vec<u32> {
    let force: Vec<u32> = at_distance(g, cid, 3)
        .into_iter()
        .take(3)
        .map(|pos| g.spawn_unit("man_at_arms", 0, pos))
        .collect();
    assert_eq!(force.len(), 3);
    for uid in &force {
        g.units.get_mut(uid).unwrap().hp = hp;
    }
    let (sealed, ring) = ring_state(g, cid);
    assert!(sealed < ring, "fixture: the ring is open ({sealed}/{ring})");
    force
}

#[test]
fn an_unwalled_city_reads_a_melee_swarm_as_fire_under_the_gene() {
    // Kish at t95: an even Man-at-Arms exchange, ring unsealed, melee only.
    let (mut g, cid) = unwalled_city(45.0);
    let force = melee_train(&mut g, cid, 100);
    let (shipped, _) = budgeting(false)
        .conversion_siege_budget(&g, 0, cid, &force)
        .unwrap();
    assert!(
        shipped.is_infinite(),
        "shipped: melee is only the finishing blow, and nothing out-fires the heal: {shipped}"
    );
    assert!(!budgeting(false).conversion_siege_ready(&g, 0, cid, &force));
    let (turns, endurance) = budgeting(true)
        .conversion_siege_budget(&g, 0, cid, &force)
        .unwrap();
    assert!(turns.is_finite(), "the swarm out-fires the heal: {turns}");
    // Three blows of ~30 past a heal of 20: 200 / 70 + 1.
    assert!(turns < 6.0, "{turns}");
    assert!(
        budgeting(true).conversion_siege_ready(&g, 0, cid, &force),
        "{turns} turns against {endurance} endurance"
    );
}

#[test]
fn the_swarm_pays_its_reply_in_endurance() {
    let (mut g, cid) = unwalled_city(45.0);
    let force = melee_train(&mut g, cid, 100);
    let (_, shipped) = budgeting(false)
        .conversion_siege_budget(&g, 0, cid, &force)
        .unwrap();
    let (_, swarm) = budgeting(true)
        .conversion_siege_budget(&g, 0, cid, &force)
        .unwrap();
    assert!(
        swarm < shipped,
        "each swing draws the city's reply: {swarm} against the shipped {shipped}"
    );
    // The reply is the budget's own exchange: the city's strength against
    // the member's, the blow it lands mirrored.
    let expected: f64 = force
        .iter()
        .map(|uid| {
            let unit = &g.units[uid];
            let reply = expected_damage(g.city_strength(cid), g.unit_strength(unit, true));
            let strike = expected_damage(g.city_ranged_strength(cid), g.unit_strength(unit, false));
            80.0 / (strike + reply)
        })
        .sum();
    assert!(
        (swarm - expected).abs() < 1e-9,
        "{swarm} against {expected}"
    );
}

#[test]
fn a_wounded_swarm_rotates_out_and_reads_no_fire() {
    let (mut g, cid) = unwalled_city(45.0);
    let force = melee_train(&mut g, cid, super::super::battle_planner::ROTATE_HP - 1);
    let (turns, _) = budgeting(true)
        .conversion_siege_budget(&g, 0, cid, &force)
        .unwrap();
    assert!(
        turns.is_infinite(),
        "members under ROTATE_HP rotate out, so they are only the finishing blow: {turns}"
    );
    // At ROTATE_HP itself, with the reply leaving ASSAULT_SURVIVOR_HP, they
    // swing.
    for uid in &force {
        g.units.get_mut(uid).unwrap().hp = super::super::battle_planner::ROTATE_HP + 10;
    }
    let reply = expected_damage(
        g.city_strength(cid),
        g.unit_strength(&g.units[&force[0]], true),
    );
    assert!(
        f64::from(super::super::battle_planner::ROTATE_HP + 10) - reply
            >= f64::from(ASSAULT_SURVIVOR_HP)
    );
    let (turns, _) = budgeting(true)
        .conversion_siege_budget(&g, 0, cid, &force)
        .unwrap();
    assert!(turns.is_finite(), "{turns}");
}

#[test]
fn a_walled_or_breached_city_budgets_the_same_with_the_gene() {
    for breached in [false, true] {
        let (mut g, cid) = walled_city();
        g.tactics.heal = true;
        if breached {
            g.cities.get_mut(&cid).unwrap().wall_hp = 0;
        }
        assert!(g.city_max_wall_hp(&g.cities[&cid]) > 0);
        let mut force = melee_train(&mut g, cid, 100);
        force.push(g.spawn_unit("catapult", 0, at_distance(&g, cid, 2)[0]));
        let off = budgeting(false).conversion_siege_budget(&g, 0, cid, &force);
        let on = budgeting(true).conversion_siege_budget(&g, 0, cid, &force);
        assert!(off.is_some(), "breached {breached}: the city is budgeted");
        assert_eq!(
            off.map(|(turns, endurance)| (turns.to_bits(), endurance.to_bits())),
            on.map(|(turns, endurance)| (turns.to_bits(), endurance.to_bits())),
            "breached {breached}: a wall pool keeps the shipped budget"
        );
    }
}

/// A Siege record for `cid` with `taker` reserved.
fn reducing(ai: &mut AdvancedAi, g: &Game, cid: u32, taker: Option<u32>) {
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Reduce,
            taker,
            entered: g.turn,
            assessed: g.turn,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
}

/// The siege step for one member beside the city, with or without the gene:
/// the city's health and the member's after it.
fn melee_step(gene: bool, strength: f64, hp: i32) -> (i32, i32, i32) {
    let (mut g, cid) = unwalled_city(strength);
    let uid = g.spawn_unit("man_at_arms", 0, ring_of(&g, cid)[0]);
    g.units.get_mut(&uid).unwrap().hp = hp;
    let mut ai = AdvancedAi::new();
    if gene {
        ai.enable_unwalled_city_takes_the_swarm();
    }
    let plan = plan_against(&g, cid);
    reducing(&mut ai, &g, cid, None);
    let city = CityView::of(&g, cid).unwrap();
    ai.siege_melee_step(&mut g, 0, uid, &city, &plan);
    (
        g.cities[&cid].hp,
        g.units.get(&uid).map_or(0, |u| u.hp),
        g.units.get(&uid).map_or(0, |u| u.attacks_left),
    )
}

#[test]
fn a_fit_member_beside_an_unwalled_city_swings_where_the_shipped_gate_holds() {
    // A City Center stronger than the Man-at-Arms (45): one blow priced
    // alone (`siege_blow`) trades badly at any roll, so the shipped ring
    // holds; the worst reply still leaves the member over its floor.
    let strength = 60.0;
    let (off_city, off_hp, _) = melee_step(false, strength, 100);
    assert_eq!((off_city, off_hp), (200, 100), "shipped: the member holds");
    let (on_city, on_hp, on_attacks) = melee_step(true, strength, 100);
    assert!(on_city < 200, "the member swings: city {on_city}");
    assert!(on_hp >= ASSAULT_SURVIVOR_HP, "and keeps its floor: {on_hp}");
    assert_eq!(on_attacks, 0);
}

#[test]
fn a_wounded_member_rotates_out_instead_of_swinging() {
    let hp = super::super::battle_planner::ROTATE_HP - 1;
    let (city, unit_hp, _) = melee_step(true, 60.0, hp);
    assert_eq!((city, unit_hp), (200, hp));
}

#[test]
fn a_member_beside_a_walled_or_breached_city_keeps_the_shipped_step() {
    for breached in [false, true] {
        let mut outcome = Vec::new();
        for gene in [false, true] {
            let (mut g, cid) = walled_city();
            g.tactics.heal = true;
            if breached {
                g.cities.get_mut(&cid).unwrap().wall_hp = 0;
            }
            std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 55.0);
            let uid = g.spawn_unit("man_at_arms", 0, ring_of(&g, cid)[0]);
            let mut ai = AdvancedAi::new();
            if gene {
                ai.enable_unwalled_city_takes_the_swarm();
            }
            let plan = plan_against(&g, cid);
            reducing(&mut ai, &g, cid, None);
            let city = CityView::of(&g, cid).unwrap();
            ai.siege_melee_step(&mut g, 0, uid, &city, &plan);
            let c = &g.cities[&cid];
            outcome.push((
                c.hp,
                c.wall_hp,
                g.units.get(&uid).map(|u| (u.hp, u.attacks_left)),
            ));
        }
        assert_eq!(outcome[0], outcome[1], "breached {breached}");
    }
}

#[test]
fn the_reserved_taker_swings_only_while_it_can_still_take_the_city() {
    for (hp, swings) in [(100, true), (80, false)] {
        let (mut g, cid) = unwalled_city(45.0);
        let uid = g.spawn_unit("man_at_arms", 0, ring_of(&g, cid)[0]);
        g.units.get_mut(&uid).unwrap().hp = hp;
        let mut ai = AdvancedAi::new();
        ai.enable_unwalled_city_takes_the_swarm();
        reducing(&mut ai, &g, cid, Some(uid));
        let city = CityView::of(&g, cid).unwrap();
        assert!(f64::from(city.hp) > taker_blow(&g, 0, uid, cid));
        let mut probe = g.speculative_clone();
        probe
            .apply(
                0,
                &Action::Attack {
                    unit: uid,
                    target: city.pos,
                },
            )
            .unwrap();
        let left = probe.units.get(&uid).map_or(0, |u| u.hp);
        assert_eq!(
            left >= STORM_TAKER_SURVIVOR_HP,
            swings,
            "fixture at {hp}: {left}"
        );
        // A member that is not the taker keeps the lower floor.
        assert!(left >= ASSAULT_SURVIVOR_HP, "fixture at {hp}: {left}");
        ai.taker_step(&mut g, 0, uid, &city);
        assert_eq!(g.cities[&cid].hp < 200, swings, "taker at {hp}");
        assert_eq!(g.cities[&cid].owner, 1);
    }
}

#[test]
fn a_swarm_blow_that_takes_the_city_captures_it() {
    let (mut g, cid) = unwalled_city(45.0);
    g.cities.get_mut(&cid).unwrap().hp = 5;
    let uid = g.spawn_unit("man_at_arms", 0, ring_of(&g, cid)[0]);
    let mut ai = AdvancedAi::new();
    ai.enable_unwalled_city_takes_the_swarm();
    reducing(&mut ai, &g, cid, None);
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.swarm_blow(&mut g, 0, uid, &city), Some(true));
    assert_eq!(g.cities[&cid].owner, 0);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Hold);
}

/// The gene is a native opt-in, off in both controllers.
#[test]
fn unwalled_city_takes_the_swarm_is_a_native_opt_in_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "unwalled-city-takes-the-swarm",
        |ai| ai.unwalled_city_takes_the_swarm,
    );
}
