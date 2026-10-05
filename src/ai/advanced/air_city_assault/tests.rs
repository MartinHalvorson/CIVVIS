use super::*;
use crate::ai::advanced::VictoryTarget;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, Vec<u32>) {
    let mut g = Game::new_full(2, 40, 24, 377300, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (10, 10), None);
    let target = g.found_city_for(1, (20, 10), None);
    g.current = 0;
    g.at_war.insert((0, 1));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 40.0);
    let cavalry = g.spawn_test_unit("cavalry", 0, (16, 10));
    let bombers = (0..2)
        .map(|_| g.spawn_test_unit("bomber", 0, (10, 10)))
        .collect();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 1,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.air_surge = true;
    (g, ai, plan, cavalry, bombers)
}

#[test]
fn cavalry_reveals_a_city_before_the_bombers_and_keeps_an_exit() {
    let (mut g, mut ai, plan, cavalry, _) = fixture();
    let target = g.cities[&plan.target_city.unwrap()].pos;
    assert!(
        !g.player_can_see(0, target),
        "remembered city starts outside sight"
    );
    let city = g.cities.get_mut(&plan.target_city.unwrap()).unwrap();
    city.wall_hp = 400;
    city.hp = 200;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(city.id, 400);
    // Give the city enough observed strength that two sorties cannot capture it.
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(city.id, 100.0);
    let before = g.log.len();
    let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
    assert!(reserved.contains(&cavalry));
    let report = ai.planned_air_city_assault().unwrap();
    assert!(report.moved_to_spot);
    assert_eq!(report.aircraft.len(), 2);
    assert_eq!(
        g.units[&cavalry].pos,
        (16, 10),
        "return before the enemy turn"
    );
    let actions: Vec<_> = g
        .log
        .iter()
        .skip(before)
        .map(|(_, action)| action)
        .collect();
    let first_bomb = actions
        .iter()
        .position(|a| matches!(a, Action::AirStrike { .. }))
        .unwrap();
    let last_bomb = actions
        .iter()
        .rposition(|a| matches!(a, Action::AirStrike { .. }))
        .unwrap();
    assert!(first_bomb > 0);
    assert!(last_bomb + 1 < actions.len(), "retreat follows the volley");
    assert!(g.cities[&plan.target_city.unwrap()].wall_hp < 400);
}

#[test]
fn a_weakened_city_is_captured_after_the_sorties() {
    let (mut g, mut ai, plan, cavalry, _) = fixture();
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().hp = 30;
    let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
    assert!(reserved.contains(&cavalry));
    assert_eq!(g.cities[&cid].owner, 0);
    assert_eq!(g.units[&cavalry].pos, (20, 10));
    assert!(
        g.legal_city_disposition_actions(0).is_empty(),
        "the prepass must not block the remaining army behind a capture prompt"
    );
    assert!(!ai.planned_air_city_assault().unwrap().aircraft.is_empty());
}

#[test]
fn no_maneuver_without_a_return_budget_aircraft_or_the_opt_in() {
    for case in ["movement", "aircraft", "opt_in", "peace", "home"] {
        let (mut g, mut ai, mut plan, cavalry, bombers) = fixture();
        match case {
            "movement" => g.units.get_mut(&cavalry).unwrap().moves_left = 2.0,
            "aircraft" => {
                for uid in bombers {
                    g.remove_unit(uid);
                }
            }
            "opt_in" => ai.air_surge = false,
            "peace" => g.at_war.clear(),
            "home" => plan.strategy = GrandStrategy::Recovery,
            _ => unreachable!(),
        }
        let before = g.log.len();
        assert!(
            ai.plan_air_city_assault(&mut g, 0, &plan).is_empty(),
            "{case}"
        );
        assert_eq!(g.log.len(), before, "{case}");
    }
}

#[test]
fn observed_frame_budget_must_cover_both_spotting_and_bombardment() {
    for left in [0, 1, 2] {
        let (mut g, mut ai, plan, _, _) = fixture();
        ai.observe_air_assault_frame(BTreeSet::new(), left);
        let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
        assert_eq!(!reserved.is_empty(), left == 2, "frames left: {left}");
    }
}

#[test]
fn spent_bombers_allow_capture_from_the_next_observed_board() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    g.relocate(cavalry, (18, 10));
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().hp = 1;
    for uid in bombers {
        g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    }
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 0);
    assert!(ai
        .plan_air_city_assault(&mut g, 0, &plan)
        .contains(&cavalry));
    assert_eq!(g.cities[&cid].owner, 0);
    assert!(ai.planned_air_city_assault().unwrap().aircraft.is_empty());
}

#[test]
fn observed_breach_is_claimed_even_after_the_plan_switches_to_another_city() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    let next = plan.target_city.unwrap();
    let breached = g.found_city_for(1, (26, 10), None);
    g.relocate(cavalry, (24, 10));
    g.cities.get_mut(&breached).unwrap().hp = 0;
    for uid in bombers {
        g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    }
    ai.observe_air_assault_frame(BTreeSet::from([(26, 10)]), 0);

    let before = g.log.len();
    let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
    assert!(reserved.contains(&cavalry));
    assert_eq!(g.cities[&breached].owner, 0);
    assert_eq!(g.cities[&next].owner, 1);
    assert!(g.log.iter().skip(before).any(|(_, action)| {
        matches!(action, Action::Attack { unit, target } if *unit == cavalry && *target == (26, 10))
    }));
    assert!(ai.planned_air_city_assault().unwrap().aircraft.is_empty());
}

#[test]
fn an_unused_bomber_does_not_delay_capture_of_an_already_breached_city() {
    let (mut g, mut ai, plan, cavalry, _) = fixture();
    g.relocate(cavalry, (18, 10));
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().hp = 1;
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 0);
    assert!(ai
        .plan_air_city_assault(&mut g, 0, &plan)
        .contains(&cavalry));
    assert_eq!(g.cities[&cid].owner, 0);
    assert!(ai.planned_air_city_assault().unwrap().aircraft.is_empty());
}

#[test]
fn an_observed_failed_breach_makes_the_cavalry_withdraw() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    g.relocate(cavalry, (18, 10));
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().wall_hp = 400;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 400);
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 100.0);
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, 100.0);
    for uid in bombers {
        g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    }
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 0);
    assert!(ai
        .plan_air_city_assault(&mut g, 0, &plan)
        .contains(&cavalry));
    assert_eq!(g.cities[&cid].owner, 1);
    assert!(g.wdist(g.units[&cavalry].pos, (20, 10)) > 2);
    assert!(ai.planned_air_city_assault().unwrap().aircraft.is_empty());
}

#[test]
fn refused_sorties_do_not_prevent_the_observed_cavalry_retreat() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    g.relocate(cavalry, (18, 10));
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().wall_hp = 400;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 400);
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 100.0);
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, 100.0);
    for uid in bombers {
        std::sync::Arc::make_mut(&mut g.blocked_strikes).insert((uid, (20, 10)));
    }
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 0);
    assert!(ai
        .plan_air_city_assault(&mut g, 0, &plan)
        .contains(&cavalry));
    assert_eq!(g.cities[&cid].owner, 1);
    assert!(g.wdist(g.units[&cavalry].pos, (20, 10)) > 2);
    assert!(ai.planned_air_city_assault().unwrap().aircraft.is_empty());
}

#[test]
fn the_ordinary_unit_loop_does_not_spend_the_reserved_spotter_again() {
    let (mut g, mut ai, plan, cavalry, _) = fixture();
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().wall_hp = 400;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 400);
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 100.0);
    ai.advanced_units(&mut g, 0, &plan);
    assert!(ai.planned_air_city_assault().is_some());
    assert_eq!(g.units[&cavalry].pos, (16, 10));
}

/// Every ranged City Center strike floors health at one, so a bombed city
/// is never at zero. Live King 20260930T211803Z: Edirne sat at
/// `walls 0/400, city 1/200` for four turns with a Tank six tiles out.
#[test]
fn a_bombed_breach_at_one_health_is_finished_by_any_melee_body() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    g.remove_unit(cavalry);
    let breached = g.found_city_for(1, (26, 10), None);
    {
        let city = g.cities.get_mut(&breached).unwrap();
        city.hp = 1;
        city.wall_hp = 0;
    }
    let infantry = g.spawn_test_unit("infantry", 0, (25, 10));
    for uid in bombers {
        g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    }
    ai.observe_air_assault_frame(BTreeSet::from([(26, 10)]), 0);
    let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
    assert!(reserved.contains(&infantry));
    assert_eq!(g.cities[&breached].owner, 0);
    assert_eq!(ai.planned_air_city_assault().unwrap().cavalry, Some(infantry));
}

/// A city's melee defence follows its owner's best unit, so the nearest
/// body is not necessarily one that survives the blow.
#[test]
fn the_strongest_body_in_reach_finishes_the_breach() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    g.remove_unit(cavalry);
    let breached = g.found_city_for(1, (26, 10), None);
    {
        let city = g.cities.get_mut(&breached).unwrap();
        city.hp = 1;
        city.wall_hp = 0;
    }
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(breached, 90.0);
    let weak = g.spawn_test_unit("horseman", 0, (25, 10));
    g.units.get_mut(&weak).unwrap().hp = 35;
    let tank = g.spawn_test_unit("tank", 0, (23, 10));
    for uid in bombers {
        g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    }
    ai.observe_air_assault_frame(BTreeSet::from([(26, 10)]), 0);
    let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
    assert_eq!(g.cities[&breached].owner, 0);
    assert!(reserved.contains(&tank));
    assert!(!reserved.contains(&weak), "the unused body keeps its turn");
    assert_eq!(g.units[&tank].pos, (26, 10));
}

#[test]
fn a_breach_nobody_can_finish_draws_the_nearest_body_in() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    g.remove_unit(cavalry);
    let breached = g.found_city_for(1, (30, 10), None);
    {
        let city = g.cities.get_mut(&breached).unwrap();
        city.hp = 1;
        city.wall_hp = 0;
    }
    let infantry = g.spawn_test_unit("infantry", 0, (25, 10));
    for uid in bombers {
        g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    }
    ai.observe_air_assault_frame(BTreeSet::from([(30, 10)]), 0);
    let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
    assert_eq!(g.cities[&breached].owner, 1, "two moves cannot reach it this turn");
    assert!(reserved.contains(&infantry));
    assert!(g.wdist(g.units[&infantry].pos, (30, 10)) < 5, "it closes in");
}

/// Live King 20260930T211803Z turn 184, frame 0: no cavalry in reach, so the
/// maneuver flew nothing and the unit loop spent the wing on field units. A
/// visible city's walls fall to the wing alone.
#[test]
fn the_wing_bombs_a_visible_city_with_no_cavalry_in_reach() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    g.remove_unit(cavalry);
    // A picket that sees the city but is no cavalry the maneuver could use.
    let picket = g.spawn_test_unit("infantry", 0, (18, 10));
    assert!(g.player_can_see(0, (20, 10)));
    ai.air_surge = false;
    ai.enable_air_surge_2();
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().wall_hp = 400;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 400);
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 2);
    let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
    for uid in &bombers {
        assert!(
            reserved.contains(uid),
            "the wing is spent on the city: reserved {reserved:?}, report {:?}, bombers {:?}",
            ai.planned_air_city_assault(),
            bombers.iter().map(|b| (g.units[b].pos, g.units[b].moves_left, g.units[b].acted, g.units[b].hp)).collect::<Vec<_>>()
        );
    }
    assert!(g.cities[&cid].wall_hp < 400);
    let report = ai.planned_air_city_assault().unwrap();
    assert_eq!(report.cavalry, None, "walls still stand; nobody walks in");
    assert_eq!(report.aircraft.len(), bombers.len());
    assert!(!reserved.contains(&picket));

    // Version one keeps its cavalry-led maneuver only.
    let (mut g, mut ai, plan, cavalry, _) = fixture();
    g.remove_unit(cavalry);
    g.spawn_test_unit("infantry", 0, (18, 10));
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 2);
    assert!(ai.plan_air_city_assault(&mut g, 0, &plan).is_empty());

    // An unseen city still waits for a spotter.
    let (mut g, mut ai, plan, cavalry, _) = fixture();
    g.remove_unit(cavalry);
    ai.air_surge = false;
    ai.enable_air_surge_2();
    ai.observe_air_assault_frame(BTreeSet::new(), 2);
    assert!(ai.plan_air_city_assault(&mut g, 0, &plan).is_empty());
}

/// Live King 2026-10-03T131343Z bombed Sheffield to one health with no taker
/// within a march, and it healed. With no land melee body within two turns
/// of the city, the cavalry-free volley is not flown.
#[test]
fn the_wing_holds_its_volley_when_nobody_can_walk_in() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    g.remove_unit(cavalry);
    ai.air_surge = false;
    ai.enable_air_surge_2();
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().wall_hp = 400;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 400);
    // A melee body far from the city: it cannot follow the breach in.
    let far = g.spawn_test_unit("infantry", 0, (6, 10));
    assert!(g.wdist(g.units[&far].pos, (20, 10)) > AIR_ASSAULT_FOLLOWUP_REACH);
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 2);
    let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
    for uid in &bombers {
        assert!(!reserved.contains(uid), "the wing stays free for other work");
    }
    assert_eq!(g.cities[&cid].wall_hp, 400, "no sortie on the city");
}

/// Live King 20260930T211803Z, Edirne at one health turns 188-191: the
/// approach was held by the Ottoman army, not merely far. The wing strikes
/// the defenders on the ring before the capture is tried again.
#[test]
fn the_wing_clears_a_guarded_breach_before_the_capture() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    g.remove_unit(cavalry);
    ai.air_surge = false;
    ai.enable_air_surge_2();
    let cid = plan.target_city.unwrap();
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.hp = 1;
        city.wall_hp = 0;
    }
    let ring: Vec<Pos> = g.nbrs((20, 10)).into_iter().collect();
    let defenders: Vec<u32> = ring
        .iter()
        .map(|pos| g.spawn_test_unit("infantry", 1, *pos))
        .collect();
    g.spawn_test_unit("tank", 0, (16, 10));
    assert!(g.player_can_see(0, (20, 10)) || {
        g.spawn_test_unit("infantry", 0, (18, 9));
        true
    });
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 2);
    let before: i32 = defenders.iter().map(|uid| g.units[uid].hp).sum();
    let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
    for uid in &bombers {
        assert!(reserved.contains(uid), "the wing flew at the approach");
    }
    let after: i32 = defenders
        .iter()
        .map(|uid| g.units.get(uid).map_or(0, |unit| unit.hp))
        .sum();
    assert!(after < before, "the defenders on the ring took the volley");
}

/// Live King 20261001T050754Z: Uppsala stood behind fallen walls from turn
/// 103 to 113 while every Swedish blow was charged to our one approaching
/// body against half its health. Toward a breach the step only has to leave
/// the body a taker, and with `air-surge-2` each hostile's single blow is
/// shared among our units in its reach.
#[test]
fn a_breach_body_closes_in_under_shared_blows() {
    let run = |v2: bool| {
        let (mut g, mut ai, plan, cavalry, bombers) = fixture();
        g.remove_unit(cavalry);
        let breached = g.found_city_for(1, (30, 10), None);
        {
            let city = g.cities.get_mut(&breached).unwrap();
            city.hp = 1;
            city.wall_hp = 0;
        }
        let body = g.spawn_test_unit("infantry", 0, (25, 10));
        // A spent partner on the far side of the ring sees the defenders
        // and shares their blows.
        let partner = g.spawn_test_unit("infantry", 0, (27, 11));
        g.units.get_mut(&partner).unwrap().moves_left = 0.0;
        let hostiles: Vec<u32> = [(28, 9), (28, 10), (28, 11)]
            .into_iter()
            .map(|at| g.spawn_test_unit("infantry", 1, at))
            .collect();
        assert!(hostiles.iter().all(|uid| g.unit_visible_to(*uid, 0)));
        for uid in bombers {
            g.units.get_mut(&uid).unwrap().moves_left = 0.0;
        }
        if v2 {
            ai.enable_air_surge_2();
        }
        ai.observe_air_assault_frame(BTreeSet::from([(30, 10)]), 0);
        // Every step closer stands in two full blows: 60 of the body's 100
        // on the full field, over the old half-health bar.
        for to in g.reachable(body) {
            let mut after = g.speculative_clone();
            if g.wdist(to, (30, 10)) < 5
                && after.apply(0, &Action::MoveTo { unit: body, to }).is_ok()
                && after.units[&body].pos == to
            {
                assert!(battle_planner::strike_danger(&after, 0, to, body) >= 50.0, "{to:?}");
            }
        }
        let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
        (reserved.contains(&body), g.wdist(g.units[&body].pos, (30, 10)))
    };
    let before = run(false);
    let after = run(true);
    assert!(!before.0, "the full field charged every blow to the body");
    assert!(after.0 && after.1 < 5, "the body closes in: {after:?}");
}

/// Domination pair seed 37140004: the wing never saw Hastings. The land
/// siege mustered beyond sight for eighteen turns and, with no cavalry in
/// reach, not one sortie flew. Any healthy soldier steps into sight and
/// stays, and the wing flies at the city it now sees.
#[test]
fn a_soldier_spots_an_unseen_city_for_the_wing() {
    let run = |v2: bool| {
        let (mut g, mut ai, plan, cavalry, _) = fixture();
        g.remove_unit(cavalry);
        let infantry = g.spawn_test_unit("infantry", 0, (16, 10));
        let target = (20, 10);
        assert!(!g.player_can_see(0, target), "the city starts unseen");
        if v2 {
            ai.enable_air_surge_2();
        }
        let walls = g.cities[&g.city_at(target).unwrap()].wall_hp;
        let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
        let city = &g.cities[&g.city_at(target).unwrap()];
        (
            reserved.contains(&infantry),
            g.player_can_see(0, target),
            city.wall_hp < walls || city.hp < 200,
        )
    };
    assert_eq!(run(false), (false, false, false), "no cavalry, no volley");
    assert_eq!(run(true), (true, true, true), "spotted and bombed");
}

/// Live King civvis-20261005T084952Z (game 114) turn 196: Curitiba at walls 0,
/// city 1 after "2 sorties with no cavalry in reach"; the bodies within eight
/// tiles could not advance, and the city stood at 20 the next turn. Under
/// `air-volley-needs-a-road` a breached city is bombed only when a melee body
/// can walk to it; standing walls are still worth the volley alone.
#[test]
fn a_breached_city_waits_for_a_body_with_a_road_in() {
    let breached = |gene: bool, walled: bool, boxed: bool| {
        let (mut g, mut ai, plan, cavalry, _) = fixture();
        g.remove_unit(cavalry);
        ai.air_surge = false;
        ai.enable_air_surge_2();
        if gene {
            ai.enable_air_volley_needs_a_road();
        }
        let cid = plan.target_city.unwrap();
        let walls = if walled { 400 } else { 0 };
        g.cities.get_mut(&cid).unwrap().wall_hp = walls;
        std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 400);
        let body = g.spawn_test_unit("infantry", 0, (16, 10));
        assert!(g.wdist(g.units[&body].pos, (20, 10)) <= AIR_ASSAULT_FOLLOWUP_REACH);
        // An archer gives the host's sight of the city and is no melee body.
        g.spawn_test_unit("archer", 0, (18, 10));
        assert!(g.player_can_see(0, (20, 10)));
        if boxed {
            let ring: Vec<_> = g.nbrs((16, 10)).into_iter().collect();
            for p in ring {
                g.map.tiles.get_mut(&p).unwrap().terrain = crate::name!("mountain");
            }
            assert_eq!(g.route_distance(body, (20, 10), 1), None);
        }
        let gate = ai.air_assault_followup_routed(&g, 0, cid, (20, 10));
        ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 2);
        let start = g.cities[&cid].hp;
        ai.plan_air_city_assault(&mut g, 0, &plan);
        (
            gate,
            g.cities[&cid].wall_hp < walls || g.cities[&cid].hp < start,
        )
    };
    assert_eq!(
        breached(true, false, true),
        (false, false),
        "no road in: the breach is not re-bombed"
    );
    assert_eq!(
        breached(true, false, false).0,
        true,
        "a body that can walk in keeps the volley"
    );
    assert_eq!(
        breached(true, true, true),
        (true, true),
        "standing walls are bombed regardless"
    );
    assert_eq!(
        breached(false, false, true),
        (true, true),
        "the gene off, eight tiles suffice"
    );
}
