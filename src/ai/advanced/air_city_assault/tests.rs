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
