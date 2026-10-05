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

/// See `close_to_staging`: a gun with no firing post walks up to the staging
/// ring instead of fortifying at home; one already on the ring waits there.
#[test]
fn a_postless_gun_closes_to_the_staging_ring() {
    let (mut g, cid) = walled_city();
    let target = g.cities[&cid].pos;
    for pos in g.wdisk(target, 10) {
        if pos == target {
            continue;
        }
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let far = super::tests::at_distance(&g, cid, 9)[0];
    let gun = g.spawn_unit("catapult", 0, far);
    let city = CityView::of(&g, cid).unwrap();
    let mut ai = AdvancedAi::new();
    assert_eq!(ai.close_to_staging(&mut g, 0, gun, &city), Some(true));
    let after = g.wdist(g.units[&gun].pos, target);
    assert!(after < 9 && after >= STAGING_FAR, "walked up to {after}");

    let near = super::tests::at_distance(&g, cid, STAGING_FAR - 1)[0];
    let waiting = g.spawn_unit("catapult", 0, near);
    assert_eq!(ai.close_to_staging(&mut g, 0, waiting, &city), None);
}

/// See `march_step`: a land unit that can embark marches around a bay over
/// dry land instead of crossing it, while the road is not too long.
#[test]
fn a_march_takes_the_land_road_around_a_bay() {
    let (mut g, cid) = walled_city();
    let target = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    // A bay three columns wide between the start and the city, open to land
    // only south of it.
    let (cx, cy) = target;
    for pos in g.map.tiles.keys().copied().collect::<Vec<_>>() {
        let (q, r) = pos;
        let dq = q - cx;
        if (-6..=-4).contains(&dq) && (cy - 8..=cy + 3).contains(&r) {
            g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
        }
    }
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    let start = (cx - 8, cy);
    let warrior = g.spawn_unit("warrior", 0, start);
    let ordinary = g
        .route_step(warrior, target, STAGING_FAR)
        .expect("an ordinary route");
    let ordinary_len = g.route_distance(warrior, target, STAGING_FAR).unwrap();
    let mut probe = g.speculative_clone();
    let mut crosses = false;
    for _ in 0..ordinary_len {
        let Some(step) = probe.route_step(warrior, target, STAGING_FAR) else {
            break;
        };
        crosses |= probe
            .map
            .get(step)
            .is_some_and(|tile| probe.rules.is_water(tile));
        probe.relocate(warrior, step);
    }
    assert!(
        crosses,
        "fixture: the ordinary route crosses the bay ({ordinary:?})"
    );
    let (dry, steps) = g
        .route_step_dry(warrior, target, STAGING_FAR, 64)
        .expect("a road around the bay");
    assert!(g.map.get(dry).is_some_and(|tile| !g.rules.is_water(tile)));
    assert!(steps > g.wdist(start, target) as usize - STAGING_FAR as usize);
    assert_eq!(march_step(&g, warrior, target, STAGING_FAR), Some(dry));
    assert_eq!(dry_march_step(&g, warrior, target, STAGING_FAR), Some(dry));
    // A road longer than the slack allows yields to the ordinary route.
    assert_eq!(g.route_step_dry(warrior, target, STAGING_FAR, 2), None);
}

/// See `siege_spotter`: an invested train that cannot see its city steps
/// its nearest healthy soldier into sight instead of investing on memory.
#[test]
fn an_invested_siege_steps_a_spotter_into_sight_of_an_unseen_city() {
    let (mut g, cid) = walled_city();
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    let city = g.cities[&cid].pos;
    for pos in g.wdisk(city, 6) {
        if pos == city {
            continue;
        }
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let start = super::tests::at_distance(&g, cid, 4)[0];
    let soldier = g.spawn_unit("swordsman", 0, start);
    assert!(
        !g.sees(&g.player_vision_frame(0), city),
        "fixture: nothing of ours sees the city"
    );
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    let group = ForceGroup {
        id: soldier,
        domain: ForceDomain::Land,
        units: vec![soldier],
        anchor: start,
        objective: city,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    };
    let view = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.siege_spotter(&g, 0, &group, &view), Some(soldier));
    assert_eq!(ai.spotter_step(&mut g, 0, soldier, &view), Some(true));
    let now = g.units[&soldier].pos;
    assert!(g.wdist(now, city) <= g.unit_sight(soldier));
    assert!(g.line_of_sight_from(now, city));
    assert_eq!(
        ai.siege_spotter(&g, 0, &group, &view),
        None,
        "the city is in sight now"
    );
}

/// See `approach`: a gun whose step-by-step route to its firing post is
/// walled off by its own soldiers walks through them toward the post
/// instead of standing still (Natal, game 30).
#[test]
fn a_posted_gun_walks_through_its_own_screen_toward_the_post() {
    let (mut g, cid, gun, _, _) = crowded_approach();
    let target = g.cities[&cid].pos;
    let post = (target.0 - 3, target.1);
    assert_eq!(siege_route_step(&g, 0, gun, post, target), None, "boxed in");
    let start = g.units[&gun].pos;
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    assert_eq!(ai.approach(&mut g, 0, gun, post, target), Some(true));
    let now = g.units[&gun].pos;
    assert!(
        g.wdist(now, post) < g.wdist(start, post),
        "the gun closed on its post: {start:?} -> {now:?}"
    );
}

/// See `HELD_BREACH_WALL_SHARE`: an invested siege whose walls are well down
/// holds through a relief that puts it under the ordinary abort share, and
/// still falls back under the deep one.
#[test]
fn a_breached_siege_holds_through_a_modest_relief_under_the_gene() {
    let run = |gene: bool, deep: bool| {
        let (mut g, cid) = walled_city();
        g.map_script = crate::setup::MapScript::Pangaea;
        g.turn = 30;
        g.at_war.insert((0, 1));
        let city = g.cities[&cid].pos;
        let max = g.city_max_wall_hp(&g.cities[&cid]);
        assert!(max > 0, "fixture: a walled city");
        g.cities.get_mut(&cid).unwrap().wall_hp = max * 6 / 10;
        let guns: Vec<_> = super::tests::at_distance(&g, cid, 2)
            .into_iter()
            .take(2)
            .map(|pos| g.spawn_unit("catapult", 0, pos))
            .collect();
        let taker = g.spawn_unit("swordsman", 0, ring_of(&g, cid)[0]);
        let force = vec![guns[0], guns[1], taker];
        let mut ai = AdvancedAi::new();
        ai.enable_siege_train();
        if gene {
            ai.enable_siege_holds_a_breach();
        }
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
                stage: SiegeStage::Reduce,
                taker: None,
                entered: 28,
                assessed: 29,
                posts: BTreeMap::new(),
                short_since: None,
            },
        );
        let strength: f64 = force.iter().map(|uid| unit_power(&g, *uid)).sum();
        let share = if deep {
            HELD_BREACH_ABORT_SHARE
        } else {
            ABORT_SHARE
        };
        // Relief arrives until the train sits under the chosen share.
        for pos in super::tests::at_distance(&g, cid, 4) {
            let view = CityView::of(&g, cid).unwrap();
            if strength < share * siege_bill(&g, 0, &view) {
                break;
            }
            g.spawn_unit("swordsman", 1, pos);
        }
        let view = CityView::of(&g, cid).unwrap();
        let bill = siege_bill(&g, 0, &view);
        assert!(strength < share * bill, "fixture: under the share");
        if !deep {
            assert!(
                strength >= HELD_BREACH_ABORT_SHARE * bill,
                "fixture: above the deep share ({strength} of {bill})"
            );
        }
        for turn in [30, 31] {
            g.turn = turn;
            ai.assess_siege(&g, 0, cid, &plan, &group);
        }
        ai.sieges[&cid].stage
    };
    assert_eq!(run(false, false), SiegeStage::Stage, "the shipped abort");
    assert_ne!(run(true, false), SiegeStage::Stage, "the breach holds");
    assert_eq!(
        run(true, true),
        SiegeStage::Stage,
        "a deep shortfall still falls back"
    );
}

/// See `conversion_siege_budget` under `siege-budget-counts-what-fires`:
/// Archers strike a city at 17 less and melee only finishes it, so two
/// Archers and a Swordsman cannot out-damage an unwalled city's heal; the
/// shipped budget reads the same force ready. Catapults, which strike
/// without the penalty, make it finite again.
#[test]
fn the_budget_counts_only_the_damage_that_fires() {
    let (mut g, cid) = walled_city();
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.wall_hp = 0;
        city.buildings
            .retain(|building| building != "walls" && building != "medieval_walls");
    }
    // Tenochtitlan's host reading in game 33: 45 on its hill.
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 45.0);
    let mut force: Vec<u32> = super::tests::at_distance(&g, cid, 2)
        .into_iter()
        .take(2)
        .map(|pos| g.spawn_unit("archer", 0, pos))
        .collect();
    force.push(g.spawn_unit("swordsman", 0, ring_of(&g, cid)[0]));
    let mut ai = AdvancedAi::new();
    let (shipped, _) = ai.conversion_siege_budget(&g, 0, cid, &force).unwrap();
    assert!(
        shipped.is_finite(),
        "the shipped budget reads it ready: {shipped}"
    );
    ai.enable_siege_budget_counts_what_fires();
    let (truthful, _) = ai.conversion_siege_budget(&g, 0, cid, &force).unwrap();
    assert!(truthful.is_infinite(), "Archers cannot out-damage the heal");
    for pos in super::tests::at_distance(&g, cid, 2)
        .into_iter()
        .skip(2)
        .take(3)
    {
        force.push(g.spawn_unit("catapult", 0, pos));
    }
    let (with_guns, _) = ai.conversion_siege_budget(&g, 0, cid, &force).unwrap();
    assert!(
        with_guns.is_finite(),
        "siege guns strike without the penalty"
    );
}

/// See `STAGING_ESCORT_BODIES`: with the gene, an escorted gun budgets one
/// reply turn instead of three, so a single raider's reach no longer holds
/// it at the edge; alone it still holds.
#[test]
fn an_escorted_staging_gun_marches_under_one_raider() {
    for trusts in [false, true] {
        let (mut g, cid) = walled_city();
        let target = g.cities[&cid].pos;
        for tile in g.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
        g.at_war.insert((0, 1));
        let start = (target.0 - 8, target.1);
        let gun = g.spawn_unit("catapult", 0, start);
        let next = march_step(&g, gun, target, STAGING_FAR).expect("a march step");
        // A raider that reaches the next step but stands beyond the gun's own
        // shot, so the gun's turn is the march.
        let escort_tiles: Vec<Pos> = g
            .nbrs(next)
            .into_iter()
            .filter(|pos| *pos != start && g.wdist(*pos, target) >= g.wdist(next, target))
            .take(3)
            .collect();
        let lair = g
            .wdisk(next, 2)
            .into_iter()
            .filter(|pos| {
                g.wdist(*pos, next) == 2 && g.wdist(*pos, start) == 3 && !escort_tiles.contains(pos)
            })
            .min()
            .expect("a tile in reach of the step, beyond the gun's shot");
        g.spawn_unit("archer", 1, lair);
        assert_eq!(escort_tiles.len(), 3, "fixture: three escort tiles");
        for pos in escort_tiles {
            g.spawn_unit("swordsman", 0, pos);
        }
        let mut field = super::super::battle_planner::DangerField::with_reach(&g, 0, true);
        field.share(&g);
        let risk = field.danger(next, gun);
        let three_turns = (100.0 - STAGING_GUN_HP_RESERVE) / STAGING_GUN_REPLY_TURNS;
        let one_turn = 100.0 - STAGING_GUN_HP_RESERVE;
        assert!(
            risk > three_turns && risk <= one_turn,
            "fixture: one raider's blow ({risk}) is between the two budgets"
        );
        let city = CityView::of(&g, cid).unwrap();
        let plan = plan_against(&g, cid);
        let mut ai = AdvancedAi::new();
        ai.enable_siege_train();
        ai.shared_danger = true;
        if trusts {
            ai.enable_staging_gun_trusts_its_escort();
        }
        ai.siege_stage_step(&mut g, 0, gun, &city, &plan);
        let closer = g.wdist(g.units[&gun].pos, target) < g.wdist(start, target);
        assert_eq!(closer, trusts, "the gun marches only under the gene");
    }
}

/// See `siege_route_step`: a gun on land whose dry approach to its post is
/// walled by its own soldiers does not embark onto the water beside it,
/// where `disembark_step` would land it again next turn (Ray, game 52). It
/// walks through its screen along the shore instead.
#[test]
fn a_posted_gun_never_embarks_around_its_own_screen() {
    let (mut g, cid, gun, _, _) = crowded_approach();
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    let target = g.cities[&cid].pos;
    let start = g.units[&gun].pos;
    for x in start.0..=target.0 {
        let tile = g.map.tiles.get_mut(&(x, target.1 - 1)).unwrap();
        tile.terrain = crate::name!("coast");
    }
    let post = (target.0 - 3, target.1);
    let water = |g: &Game, pos: Pos| g.rules.is_water(g.map.get(pos).unwrap());
    // Fixture: the plain route around the screen leads across the water.
    let wet = g.route_step(gun, post, 0).expect("a route by water");
    assert!(water(&g, wet), "fixture: the open route embarks at {wet:?}");
    assert_eq!(
        siege_route_step(&g, 0, gun, post, target),
        None,
        "on land, the post route keeps to land"
    );
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.approach(&mut g, 0, gun, post, target);
    let now = g.units[&gun].pos;
    assert!(!water(&g, now), "the gun stands on dry ground: {now:?}");
    assert!(!g.is_embarked(&g.units[&gun]));
}

/// See `siege_posts_keeping`: a melee unit still walking in keeps last
/// turn's ring post while it stays free and reachable, rather than taking
/// whichever tile the spread-first order prefers this turn (the Siege Tower
/// carrier at Yaroslavl, game 62). An occupied post is given up.
#[test]
fn a_walking_unit_keeps_last_turns_ring_post() {
    let (mut g, cid) = super::tests::walled_city();
    let ring = super::tests::ring_of(&g, cid);
    let start = super::tests::at_distance(&g, cid, 3)[0];
    let walker = g.spawn_unit("warrior", 0, start);
    let city = CityView::of(&g, cid).unwrap();
    let fresh = siege_posts(&g, 0, &city, &[walker], None)[&walker];
    let kept = ring
        .iter()
        .copied()
        .find(|pos| *pos != fresh && siege_route_step(&g, 0, walker, *pos, city.pos).is_some())
        .expect("another reachable ring tile");
    let previous = BTreeMap::from([(walker, kept)]);
    assert_eq!(
        siege_posts_keeping(&g, 0, &city, &[walker], None, &previous)[&walker],
        kept
    );
    // Taken by someone else, the old post gives way to this turn's choice.
    g.spawn_unit("warrior", 0, kept);
    assert_ne!(
        siege_posts_keeping(&g, 0, &city, &[walker], None, &previous)[&walker],
        kept
    );
}

/// A one-tile defile toward the city with one of ours in its gap, and a
/// single open pocket beside the marching gun's start. The router may stop
/// only on its first step, so from the start it turns into the pocket, which
/// is no nearer the city, and from the pocket it turns back. Live King
/// civvis-20261005T003728Z (game 89): Mashhad's guns stepped between two
/// tiles behind a ridge for turns. `in_pocket` starts the gun in the pocket.
fn defile_with_a_pocket(in_pocket: bool) -> (Game, u32, u32, u32, Pos, Pos) {
    let (mut g, cid) = walled_city();
    let target = g.cities[&cid].pos;
    let start = (target.0 - 7, target.1);
    let occupied = (target.0 - 6, target.1);
    let pocket = (start.0, start.1 + 1);
    for tile in g.map.tiles.values_mut() {
        let lane = tile.pos.1 == target.1 && (start.0..=target.0).contains(&tile.pos.0);
        tile.terrain = if lane || tile.pos == pocket {
            crate::name!("grassland")
        } else {
            crate::name!("mountain")
        };
        tile.feature = None;
        tile.hills = false;
    }
    let gun = g.spawn_unit("catapult", 0, if in_pocket { pocket } else { start });
    let screen = g.spawn_unit("swordsman", 0, occupied);
    (g, cid, gun, screen, start, pocket)
}

/// Without the gene the march step goes into the pocket, and from the
/// pocket back; with `staging-column-passes-through` the gun crosses its own
/// screen and stands nearer the city, the screen unmoved.
#[test]
fn staging_column_crosses_the_friend_in_the_gap_instead_of_stepping_aside() {
    let (g, cid, gun, _, start, pocket) = defile_with_a_pocket(false);
    let target = g.cities[&cid].pos;
    assert_eq!(march_step(&g, gun, target, STAGING_FAR), Some(pocket));
    assert!(
        g.wdist(pocket, target) >= g.wdist(start, target),
        "the pocket is no nearer"
    );
    let (g, _, gun, _, start, _) = defile_with_a_pocket(true);
    assert_eq!(
        march_step(&g, gun, target, STAGING_FAR),
        Some(start),
        "the two-cycle"
    );

    let run = |gene: bool| {
        let (mut g, cid, gun, screen, _, _) = defile_with_a_pocket(false);
        let city = CityView::of(&g, cid).unwrap();
        let plan = plan_against(&g, cid);
        let mut ai = AdvancedAi::new();
        ai.enable_siege_train();
        if gene {
            ai.enable_staging_column_passes_through();
        }
        let screen_pos = g.units[&screen].pos;
        ai.siege_stage_step(&mut g, 0, gun, &city, &plan);
        assert_eq!(g.units[&screen].pos, screen_pos, "the screen stays put");
        (g.units[&gun].pos, g.wdist(g.units[&gun].pos, city.pos))
    };
    let (off_pos, off_distance) = run(false);
    assert_eq!(off_pos, pocket, "without the gene the march steps aside");
    let (on_pos, on_distance) = run(true);
    assert!(
        on_distance < off_distance && on_distance > CITY_STRIKE_RANGE,
        "the gene crosses the screen to a nearer tile outside the city's reach: {on_pos:?} at {on_distance}"
    );
}

/// With an open lane the march step already closes; the gene leaves the
/// turn exactly as it was.
#[test]
fn staging_column_gene_leaves_an_open_march_alone() {
    let run = |gene: bool| {
        let (mut g, cid, gun, screen, _, _) = defile_with_a_pocket(false);
        g.remove_unit(screen);
        let city = CityView::of(&g, cid).unwrap();
        let plan = plan_against(&g, cid);
        let mut ai = AdvancedAi::new();
        ai.enable_siege_train();
        if gene {
            ai.enable_staging_column_passes_through();
        }
        ai.siege_stage_step(&mut g, 0, gun, &city, &plan);
        (g.units[&gun].pos, g.units[&gun].moves_left.to_bits())
    };
    let (on, off) = (run(true), run(false));
    assert_eq!(on, off);
}
