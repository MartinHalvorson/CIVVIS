//! `unwalled-city-spares-the-train`: a city with no standing wall cannot
//! strike (`Game::city_can_strike`), so the siege's entry reads charge the
//! train's endurance none of its fire unless a walled Encampment shoots in
//! its place. Over the 10-13 Immortal control runs 21 objectives stood
//! unwalled at full health during a muster, a shooter in range on 2, and 30
//! of the 70 muster holds read a finite budget short of an endurance charged
//! a strike at the owner's best ranged strength.

use super::tests::{at_distance, plan_against, walled_city};
use super::*;

/// `walled_city` with no wall pool (the host's `walls 0/0`), a City Center
/// of 30, the owner's best shooter at 40, at war on open grassland.
fn open_city() -> (Game, u32) {
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
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 0);
    assert_eq!(g.city_max_wall_hp(&g.cities[&cid]), 0, "no wall pool");
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 30.0);
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, 40.0);
    // A train mustered out of sight reads the city as last seen.
    if let Some(memory) = g.players[0].remembered_cities.get_mut(&cid) {
        memory.wall_hp = 0;
    }
    (g, cid)
}

/// The deployed budget, with or without the gene.
fn entry_reader(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_siege_budget_counts_what_fires();
    ai.enable_siege_positive_damage_budget();
    if gene {
        ai.enable_unwalled_city_spares_the_train();
    }
    ai
}

/// Two Crossbowmen at range and a Man-at-Arms to take the city.
fn fire_train(g: &mut Game, cid: u32, shooters_at: i32, taker_at: i32) -> Vec<u32> {
    let mut force: Vec<u32> = at_distance(g, cid, shooters_at)
        .into_iter()
        .take(2)
        .map(|pos| g.spawn_unit("crossbowman", 0, pos))
        .collect();
    force.push(g.spawn_unit("man_at_arms", 0, at_distance(g, cid, taker_at)[0]));
    assert_eq!(force.len(), 3);
    force
}

#[test]
fn an_unwalled_city_charges_the_entry_no_strike_under_the_gene() {
    let (mut g, cid) = open_city();
    let force = fire_train(&mut g, cid, 2, 3);
    let off = entry_reader(false);
    let shipped = off
        .conversion_siege_budget_within(&g, 0, cid, &force, 5)
        .unwrap();
    assert_eq!(
        off.conversion_siege_entry_budget(&g, 0, cid, &force, 5),
        Some(shipped)
    );
    let (turns, endurance) = shipped;
    assert!(
        turns.is_finite() && turns > endurance * 0.8,
        "fixture: the fire out-paces the heal but the phantom strike reads it short: \
         {turns} turns against {endurance}"
    );
    assert!(!off.conversion_siege_ready(&g, 0, cid, &force));

    let on = entry_reader(true);
    let (entry_turns, entry_endurance) = on
        .conversion_siege_entry_budget(&g, 0, cid, &force, 5)
        .unwrap();
    assert_eq!(entry_turns, turns, "the fire and the heal are unchanged");
    // No strike: each member stands its health above 20 at the floor of one.
    let expected: f64 = force
        .iter()
        .map(|uid| f64::from(g.units[uid].hp) - 20.0)
        .sum();
    assert!(
        (entry_endurance - expected).abs() < 1e-9,
        "{entry_endurance} against {expected}"
    );
    assert!(on.conversion_siege_ready(&g, 0, cid, &force));
    assert_eq!(
        on.conversion_siege_budget_within(&g, 0, cid, &force, 5),
        Some(shipped),
        "the production reads keep the shipped budget"
    );
}

#[test]
fn a_walled_city_reads_the_shipped_entry_budget() {
    let (mut g, cid) = walled_city();
    g.at_war.insert((0, 1));
    assert!(g.cities[&cid].wall_hp > 0, "fixture: walls stand");
    let force = fire_train(&mut g, cid, 2, 3);
    let shipped = entry_reader(false).conversion_siege_entry_budget(&g, 0, cid, &force, 5);
    assert!(shipped.is_some());
    let on = entry_reader(true);
    assert_eq!(
        on.conversion_siege_entry_budget(&g, 0, cid, &force, 5),
        shipped
    );
    assert_eq!(
        on.conversion_siege_ready(&g, 0, cid, &force),
        entry_reader(false).conversion_siege_ready(&g, 0, cid, &force)
    );
}

#[test]
fn a_walled_encampment_keeps_the_strike_until_it_is_pillaged() {
    let (mut g, cid) = open_city();
    let force = fire_train(&mut g, cid, 2, 3);
    let enc = at_distance(&g, cid, 1)
        .into_iter()
        .find(|pos| g.unit_ids_at(*pos).is_empty())
        .unwrap();
    g.map.tiles.get_mut(&enc).unwrap().district = Some(crate::name!("encampment"));
    g.map.tiles.get_mut(&enc).unwrap().owner_city = Some(cid);
    let city = g.cities.get_mut(&cid).unwrap();
    city.districts.insert(crate::name!("encampment"), enc);
    city.encampment_hp = 100;
    city.encampment_wall_hp = 100;
    let shipped = entry_reader(false).conversion_siege_entry_budget(&g, 0, cid, &force, 5);
    let on = entry_reader(true);
    assert_eq!(
        on.conversion_siege_entry_budget(&g, 0, cid, &force, 5),
        shipped,
        "a walled Encampment strikes in the city's place"
    );
    g.cities.get_mut(&cid).unwrap().encampment_pillaged = true;
    let (_, spared) = on
        .conversion_siege_entry_budget(&g, 0, cid, &force, 5)
        .unwrap();
    assert!(
        spared > shipped.unwrap().1,
        "a pillaged Encampment strikes nobody"
    );
}

/// The train mustered seven tiles out, in a force group, the siege in Stage.
fn mustered(gene: bool) -> (Game, u32, AdvancedAi, ForceGroup, StrategicPlan) {
    let (mut g, cid) = open_city();
    let units = fire_train(&mut g, cid, 7, 7);
    let city = g.cities[&cid].pos;
    let group = ForceGroup {
        id: units[0],
        domain: ForceDomain::Land,
        units: units.clone(),
        anchor: g.units[&units[0]].pos,
        objective: city,
        focus_target: None,
        posture: super::super::ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    };
    let plan = plan_against(&g, cid);
    let mut ai = entry_reader(gene);
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    ai.enable_stage_musters_out_of_reach();
    ai.force_groups.push(group.clone());
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Stage,
            taker: None,
            entered: 29,
            assessed: 29,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
    (g, cid, ai, group, plan)
}

#[test]
fn the_mustered_train_closes_on_an_unwalled_city_only_under_the_gene() {
    for gene in [false, true] {
        let (g, cid, mut ai, group, plan) = mustered(gene);
        ai.assess_siege(&g, 0, cid, &plan, &group);
        assert_eq!(
            ai.stage_muster_ready.get(&cid).copied(),
            Some(gene),
            "gene {gene}: the train closes only when the phantom strike is gone"
        );
    }
}

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

/// On the staging ring, the train invests under the gene and its shooters
/// strike the city; the melee taker holds its blow against a healthy city,
/// as it does today (`siege_blow`), and so takes no reply.
#[test]
fn the_staged_train_invests_and_fires_while_the_melee_holds() {
    let mut fired = Vec::new();
    for gene in [false, true] {
        let (mut g, cid) = open_city();
        let force = fire_train(&mut g, cid, 2, 3);
        let taker = force[2];
        let plan = plan_against(&g, cid);
        let mut ai = entry_reader(gene);
        ai.enable_siege_train();
        ai.rebuild_force_groups(&g, 0, &plan);
        let group = ai
            .force_groups
            .iter()
            .find(|group| group.units.contains(&taker))
            .cloned()
            .expect("the train forms a force");
        ai.assess_siege(&g, 0, cid, &plan, &group);
        let stage = ai.sieges[&cid].stage;
        if gene {
            assert_ne!(stage, SiegeStage::Stage, "the train invests");
        } else {
            assert_eq!(stage, SiegeStage::Stage, "today the train holds in Stage");
        }
        play(&mut ai, &mut g, 0, &plan);
        fired.push(200 - g.cities[&cid].hp);
        assert_eq!(g.cities[&cid].owner, 1);
        assert_eq!(
            g.units[&taker].hp, 100,
            "gene {gene}: the taker holds its blow against a healthy city"
        );
    }
    assert_eq!(fired[0], 0, "a Stage train does not fire");
    assert!(
        fired[1] > 0,
        "the invested shooters strike the city: {fired:?}"
    );
}

/// The gene is a native opt-in, off in both controllers.
#[test]
fn unwalled_city_spares_the_train_is_a_native_opt_in_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "unwalled-city-spares-the-train",
        |ai| ai.unwalled_city_spares_the_train,
    );
}
