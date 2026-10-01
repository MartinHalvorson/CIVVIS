use super::super::ForcePosture;
use super::tests::{plan_against, ring_of, walled_city};
use super::*;

fn crowded_approach() -> (Game, u32, u32, u32, Pos) {
    let (mut g, cid) = walled_city();
    let target = g.cities[&cid].pos;
    let start = (target.0 - 7, target.1);
    let occupied = (target.0 - 6, target.1);
    let landing = (target.0 - 5, target.1);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if tile.pos.1 == target.1 && (start.0..=target.0).contains(&tile.pos.0) {
            crate::name!("grassland")
        } else {
            crate::name!("mountain")
        };
        tile.feature = None;
        tile.hills = false;
    }
    let gun = g.spawn_unit("catapult", 0, start);
    let screen = g.spawn_unit("swordsman", 0, occupied);
    assert!(!g.can_move(gun, occupied));
    assert_eq!(g.route_step(gun, target, STAGING_FAR), None);
    assert_eq!(
        g.pass_through_destination(gun, target, STAGING_FAR),
        Some(landing)
    );
    (g, cid, gun, screen, landing)
}

#[test]
fn staging_gun_crosses_a_friendly_column_to_an_open_ring_tile() {
    let (mut g, cid, gun, screen, landing) = crowded_approach();
    let screen_pos = g.units[&screen].pos;
    let city = CityView::of(&g, cid).unwrap();
    let plan = plan_against(&g, cid);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    assert!(ai.siege_stage_step(&mut g, 0, gun, &city, &plan));
    assert_eq!(
        g.units[&gun].pos, landing,
        "staging must not spend the turn parked behind its own screen"
    );
    assert_eq!(g.units[&screen].pos, screen_pos);
    assert!(g.wdist(g.units[&gun].pos, city.pos) > CITY_STRIKE_RANGE);
}

#[test]
fn staging_gun_without_enough_movement_cannot_stop_on_its_screen() {
    let (mut g, cid, gun, screen, _) = crowded_approach();
    g.units.get_mut(&gun).unwrap().moves_left = 1.0;
    let start = g.units[&gun].pos;
    let city = CityView::of(&g, cid).unwrap();
    let plan = plan_against(&g, cid);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.siege_stage_step(&mut g, 0, gun, &city, &plan);
    assert_eq!(g.units[&gun].pos, start);
    assert_ne!(g.units[&gun].pos, g.units[&screen].pos);
}

#[test]
fn staging_gun_escapes_a_hostile_city_firing_lane() {
    let (mut g, cid) = walled_city();
    let target = g.cities[&cid].pos;
    let start = (target.0 - 7, target.1);
    let outpost = (start.0, start.1 + 2);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let outpost_id = g.found_city_for(1, outpost, None);
    g.cities.get_mut(&outpost_id).unwrap().wall_hp = 100;
    g.at_war.insert((0, 1));
    let gun = g.spawn_unit("catapult", 0, start);
    g.units.get_mut(&gun).unwrap().hp = 45;
    let risk_before = super::super::battle_planner::strike_danger(&g, 0, start, gun);
    assert!(risk_before > (45.0 - STAGING_GUN_HP_RESERVE) / STAGING_GUN_REPLY_TURNS);

    let city = CityView::of(&g, cid).unwrap();
    let plan = plan_against(&g, cid);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    assert!(ai.siege_stage_step(&mut g, 0, gun, &city, &plan));
    let after = g.units[&gun].pos;
    assert_ne!(after, start);
    assert!(super::super::battle_planner::strike_danger(&g, 0, after, gun) < risk_before);
}

#[test]
fn a_wall_rebuild_sends_an_unproductive_melee_siege_back_to_staging() {
    let (mut g, cid) = walled_city();
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    assert!(!g.is_arena());
    let units: Vec<u32> = ring_of(&g, cid)
        .into_iter()
        .take(3)
        .map(|pos| g.spawn_unit("modern_armor", 0, pos))
        .collect();
    assert_eq!(units.len(), 3);
    let city = CityView::of(&g, cid).unwrap();
    let strength: f64 = units.iter().map(|uid| unit_power(&g, *uid)).sum();
    assert!(strength >= ABORT_SHARE * siege_bill(&g, 0, &city));
    let group = ForceGroup {
        id: units[0],
        domain: ForceDomain::Land,
        units: units.clone(),
        anchor: g.units[&units[0]].pos,
        objective: city.pos,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    };
    let plan = plan_against(&g, cid);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_positive_damage_budget();
    ai.force_groups.push(group.clone());
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Reduce,
            taker: None,
            entered: 25,
            assessed: 29,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
    let mut off = ai.clone();
    off.disable_siege_positive_damage_budget();
    off.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(off.sieges[&cid].stage, SiegeStage::Reduce);

    // One short assessment holds (`ABORT_PATIENCE`); the second falls back.
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Reduce);
    g.turn = 31;
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Stage);
}

#[test]
fn an_invested_siege_keeps_its_firing_posts_through_a_small_budget_dip() {
    let (mut g, cid) = walled_city();
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    let city = g.cities[&cid].pos;
    let guns: Vec<_> = super::tests::at_distance(&g, cid, 2)
        .into_iter()
        .take(2)
        .map(|pos| g.spawn_unit("catapult", 0, pos))
        .collect();
    assert_eq!(guns.len(), 2);
    let taker_pos = ring_of(&g, cid)[0];
    let taker = g.spawn_unit("swordsman", 0, taker_pos);
    let force = [guns[0], guns[1], taker];
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_positive_damage_budget();

    // A barely spent volley may cross the strict 80% entry margin even though
    // the same train can still finish before its estimated endurance expires.
    let mut found = false;
    'budget: for hp in (30..=100).step_by(5) {
        for uid in force {
            g.units.get_mut(&uid).unwrap().hp = hp;
        }
        for wall in (10..=100).step_by(5) {
            g.cities.get_mut(&cid).unwrap().wall_hp = wall;
            let view = CityView::of(&g, cid).unwrap();
            let strength: f64 = force.iter().map(|uid| unit_power(&g, *uid)).sum();
            if strength < siege_bill(&g, 0, &view) {
                continue;
            }
            if ai
                .conversion_siege_budget(&g, 0, cid, &force)
                .is_some_and(|(turns, endurance)| turns > endurance * 0.8 && turns <= endurance)
            {
                found = true;
                break 'budget;
            }
        }
    }
    assert!(found, "the train must straddle the entry and exit margins");
    let damaged_wall = g.cities[&cid].wall_hp;
    let full_wall = CityView::of(&g, cid).unwrap().wall_max;
    let group = ForceGroup {
        id: guns[0],
        domain: ForceDomain::Land,
        units: force.to_vec(),
        anchor: g.units[&guns[0]].pos,
        objective: city,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    };
    let plan = plan_against(&g, cid);
    g.cities.get_mut(&cid).unwrap().wall_hp = full_wall;
    assert!(!ai.conversion_siege_ready(&g, 0, cid, &force));
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Stage);
    g.turn += 1;
    g.cities.get_mut(&cid).unwrap().wall_hp = damaged_wall;
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Invest);
    g.turn += 1;
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Invest);
}

/// One assessment short of the abort share does not drop an invested train
/// back to Stage; two consecutive ones do. A defender walking into the
/// bill's radius for one turn no longer restarts the Invest clock.
#[test]
fn a_one_turn_bill_spike_does_not_drop_an_invested_siege() {
    let (mut g, cid) = walled_city();
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    let city = g.cities[&cid].pos;
    let guns: Vec<_> = super::tests::at_distance(&g, cid, 2)
        .into_iter()
        .take(2)
        .map(|pos| g.spawn_unit("catapult", 0, pos))
        .collect();
    let taker = g.spawn_unit("swordsman", 0, ring_of(&g, cid)[0]);
    let force = vec![guns[0], guns[1], taker];
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    let group = ForceGroup {
        id: guns[0],
        domain: ForceDomain::Land,
        units: force.clone(),
        anchor: g.units[&guns[0]].pos,
        objective: city,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    };
    let plan = plan_against(&g, cid);
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Invest,
            taker: None,
            entered: 28,
            assessed: 29,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
    // A relief column inside the bill's radius.
    let relief: Vec<u32> = super::tests::at_distance(&g, cid, 4)
        .into_iter()
        .take(4)
        .map(|pos| g.spawn_unit("swordsman", 1, pos))
        .collect();
    let view = CityView::of(&g, cid).unwrap();
    let strength: f64 = force.iter().map(|uid| unit_power(&g, *uid)).sum();
    assert!(
        strength < ABORT_SHARE * siege_bill(&g, 0, &view),
        "fixture: the relief puts the train under the abort share"
    );
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(
        ai.sieges[&cid].stage,
        SiegeStage::Invest,
        "one short turn holds"
    );
    assert_eq!(ai.sieges[&cid].short_since, Some(30));

    // The relief walks off: the clock clears and the train stays invested.
    for uid in &relief {
        g.remove_unit(*uid);
    }
    g.turn = 31;
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_ne!(ai.sieges[&cid].stage, SiegeStage::Stage);
    assert_eq!(ai.sieges[&cid].short_since, None);

    // A relief that stays two assessments drops the train to Stage.
    for pos in super::tests::at_distance(&g, cid, 4).into_iter().take(4) {
        g.spawn_unit("swordsman", 1, pos);
    }
    for turn in [32, 33] {
        g.turn = turn;
        ai.assess_siege(&g, 0, cid, &plan, &group);
    }
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Stage);
}
