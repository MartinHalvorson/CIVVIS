use super::super::{GrandStrategy, StrategicPlan};
use super::*;
use crate::name;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 936000, 1000, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let first = g.found_city_for(0, (6, 12), None);
    let second = g.found_city_for(0, (12, 12), None);
    let target = g.found_city_for(1, (20, 12), None);
    for cid in [first, second] {
        g.cities.get_mut(&cid).unwrap().pop = 12;
    }
    let ancestors = g.rules.tech_ancestors[AIR_SURGE_GOAL_TECH].clone();
    for tech in ancestors {
        g.players[0].techs.insert(Name::new(&tech));
    }
    g.players[0].techs.insert(Name::new(AIR_SURGE_GOAL_TECH));
    g.players[0]
        .strategic_resources
        .insert(name!("aluminum"), 400.0);
    g.players[0].gold = 10000.0;
    g.players[0].gold_per_turn = 100.0;
    g.players[0].met.insert(1);
    g.at_war.clear();
    g.turn = 150;
    g.current = 0;
    let mut ai = AdvancedAi::new();
    ai.enable_air_surge_2();
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: target,
        objective_pos: g.cities[&target].pos,
        body_unit: name!("musketman"),
        body_is_cavalry: false,
        opened_at_war: false,
        phase: AirSurgePhase::Arm,
        appointed_turn: 140,
        tech_turn: Some(150),
        declared_turn: None,
        last_reviewed_turn: 150,
        recovery_assessments: 0,
    });
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 150,
        rush: false,
    };
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    (g, ai, plan, first, second)
}

fn stealth_fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let (mut g, ai, plan, first, second) = fixture();
    for tech in g.rules.tech_ancestors["stealth_technology"].clone() {
        g.players[0].techs.insert(Name::new(&tech));
    }
    g.players[0].techs.insert(name!("stealth_technology"));
    for cid in [first, second] {
        let pos = g.district_sites(cid, name!("aerodrome"))[0];
        g.cities
            .get_mut(&cid)
            .unwrap()
            .districts
            .insert(name!("aerodrome"), pos);
        g.map.tiles.get_mut(&pos).unwrap().district = Some(name!("aerodrome"));
    }
    (g, ai, plan, first, second)
}

#[test]
fn launch_escort_starts_before_the_third_bomber() {
    let (mut g, mut ai, _, first, second) = fixture();
    g.players[0].civ = "Gran Colombia".to_string();
    g.players[0].techs.insert(name!("military_science"));
    g.players[0]
        .strategic_resources
        .insert(name!("horses"), 100.0);
    ai.air_surge_plan.as_mut().unwrap().body_unit = name!("llanero");
    ai.air_surge_plan.as_mut().unwrap().body_is_cavalry = true;
    for cid in [first, second] {
        let source = *g.cities[&cid]
            .owned_tiles
            .iter()
            .find(|pos| **pos != g.cities[&cid].pos)
            .unwrap();
        let tile = g.map.tiles.get_mut(&source).unwrap();
        tile.resource = Some(name!("aluminum"));
        tile.improvement = Some(name!("mine"));
    }
    let field = g.district_sites(first, name!("aerodrome"))[0];
    g.cities
        .get_mut(&first)
        .unwrap()
        .districts
        .insert(name!("aerodrome"), field);
    g.map.tiles.get_mut(&field).unwrap().district = Some(name!("aerodrome"));
    for cid in [first, second] {
        g.spawn_test_unit("bomber", 0, g.cities[&cid].pos);
    }
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 4);
    assert_eq!(ai.air_surge_status.bombers_committed, 2);
    assert_eq!(ai.air_surge_status.bodies_committed, 0);
    assert!(g.can_produce(
        0,
        first,
        &Item::Unit {
            unit: name!("llanero")
        }
    ));
    assert!(g.can_produce(
        0,
        first,
        &Item::Unit {
            unit: name!("bomber")
        }
    ));

    assert!(ai.air_surge_production(&mut g, 0));
    let escort_city = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| !g.cities[cid].queue.is_empty())
        .unwrap();
    assert_eq!(
        g.cities[&escort_city].queue.first(),
        Some(&Item::Unit {
            unit: name!("llanero")
        }),
        "two committed bombers must release the first land capturer"
    );

    g.cities.get_mut(&escort_city).unwrap().queue.clear();
    for pos in [(7, 12), (8, 12)] {
        g.spawn_test_unit("llanero", 0, pos);
    }
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(ai.air_surge_status.bodies_committed, 2);
    assert!(ai.air_surge_production(&mut g, 0));
    assert_eq!(
        g.cities[&first].queue.first(),
        Some(&Item::Unit {
            unit: name!("bomber")
        }),
        "the third bomber resumes after the launch escort is covered"
    );
}

#[test]
fn active_package_keeps_existing_and_queued_bomber_generations() {
    let (mut g, mut ai, plan, first, second) = stealth_fixture();
    g.spawn_test_unit("jet_bomber", 0, g.cities[&first].pos);
    g.spawn_test_unit("fighter", 0, g.cities[&second].pos);
    let queued = Item::Formation {
        unit: name!("jet_bomber"),
        formation: 1,
    };
    g.cities.get_mut(&second).unwrap().queue = vec![queued.clone()];
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(
        ai.air_surge_status.bombers, 1,
        "the upgraded aircraft still supplies the wing"
    );
    assert_eq!(
        ai.air_surge_status.bombers_committed, 2,
        "a queued successor supplies one aircraft, including a formation"
    );
    assert_eq!(ai.air_surge_standing_package(&g, 0).1, 1);
    let legal_queue = Item::Unit {
        unit: name!("jet_bomber"),
    };
    g.cities.get_mut(&second).unwrap().queue = vec![legal_queue.clone()];
    // Preserve a nearly finished successor, like the observed native queue.
    g.cities.get_mut(&second).unwrap().production =
        g.item_cost_for_city(0, second, &legal_queue) - 10.0;
    let value = ai.production_value(&g, 0, second, &legal_queue, &plan, &ai.counts(&g, 0));
    assert!(
        value > 7000.0,
        "the final successor queue lost package priority: {value}"
    );
    ai.preempt_margin = 1.25;
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&second].queue.first(), Some(&legal_queue));
    g.cities.get_mut(&second).unwrap().queue = vec![Item::Unit {
        unit: name!("bomber"),
    }];
    g.cities.get_mut(&second).unwrap().production = 0.0;
    assert_eq!(
        ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap())
            .bombers_committed,
        2
    );
}

#[test]
fn two_upgraded_bombers_fill_the_launch_quota_without_ordering_a_third() {
    let (mut g, mut ai, _, first, second) = stealth_fixture();
    for cid in [first, second] {
        g.spawn_test_unit("jet_bomber", 0, g.cities[&cid].pos);
    }
    for pos in [(6, 12), (7, 12), (8, 12), (9, 12)] {
        g.spawn_test_unit("modern_armor", 0, pos);
    }
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
    assert!(ai.air_surge_status.wing_ready());
    assert!(!ai.air_surge_production(&mut g, 0));
    assert!(g
        .player_city_ids(0)
        .into_iter()
        .all(|cid| g.cities[&cid].queue.is_empty()));
}

#[test]
fn mechanized_infantry_can_escort_a_modern_armor_surge() {
    let (mut g, mut ai, mut strategy, first, second) = stealth_fixture();
    let target = ai.air_surge_plan.as_ref().unwrap().objective_city;
    let surge = ai.air_surge_plan.as_mut().unwrap();
    surge.body_unit = name!("modern_armor");
    surge.phase = AirSurgePhase::Exploit;

    for cid in [first, second] {
        g.spawn_test_unit("jet_bomber", 0, g.cities[&cid].pos);
    }
    g.spawn_test_unit("modern_armor", 0, (7, 12));
    g.spawn_test_unit("mechanized_infantry", 0, (8, 12));
    g.spawn_test_unit("tank", 0, (9, 12));
    g.spawn_test_unit("at_crew", 0, (10, 12));
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert!(ai.air_surge_status.wing_ready());
    assert_eq!(
        ai.air_surge_status.bodies, 3,
        "late land capturers should count across upgrade branches, but weaker units should not"
    );
    assert!(ai.air_surge_status.escort_ready());

    strategy.target_city = None;
    ai.apply_air_surge_to_strategy(&g, 0, &mut strategy);
    assert_eq!(strategy.target_city, Some(target));
}

#[test]
fn active_package_trains_a_legal_successor_after_the_base_bomber_is_obsolete() {
    let (mut g, mut ai, _, first, _) = stealth_fixture();
    assert!(g.can_produce(
        0,
        first,
        &Item::Unit {
            unit: name!("jet_bomber")
        }
    ));
    assert!(!g.can_produce(
        0,
        first,
        &Item::Unit {
            unit: name!("bomber")
        }
    ));
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert!(ai.air_surge_production(&mut g, 0));
    assert!(
        g.player_city_ids(0)
            .into_iter()
            .any(|cid| g.cities[&cid].queue.first()
                == Some(&Item::Unit {
                    unit: name!("jet_bomber")
                })),
        "the active package must use a trainable bomber generation"
    );
}

#[test]
fn two_upgraded_bombers_use_their_range_without_treating_fighters_as_a_wing() {
    let (mut g, mut ai, _, first, second) = stealth_fixture();
    let objective = (12 + g.rules.units["bomber"].range + 2, 12);
    let closest = [first, second]
        .into_iter()
        .map(|cid| g.wdist(g.cities[&cid].pos, objective))
        .min()
        .unwrap();
    assert!(closest > g.rules.units["bomber"].range);
    assert!(closest <= g.rules.units["jet_bomber"].range);
    ai.air_surge_plan.as_mut().unwrap().objective_pos = objective;
    for cid in [first, second] {
        g.spawn_test_unit("jet_bomber", 0, g.cities[&cid].pos);
    }
    let status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(status.bombers, 2);
    assert!(
        status.wing_ready(),
        "two upgraded bombers can reach the appointed objective"
    );
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    for cid in [first, second] {
        g.spawn_test_unit("fighter", 0, g.cities[&cid].pos);
    }
    let status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(status.bombers, 0);
    assert!(!status.wing_ready());
}

#[test]
fn a_mixed_wing_needs_two_aircraft_with_enough_range() {
    let (mut g, mut ai, _, first, second) = stealth_fixture();
    let objective = (12 + g.rules.units["bomber"].range + 2, 12);
    ai.air_surge_plan.as_mut().unwrap().objective_pos = objective;
    g.spawn_test_unit("bomber", 0, g.cities[&first].pos);
    g.spawn_test_unit("jet_bomber", 0, g.cities[&second].pos);
    let status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(status.bombers, 2);
    assert!(
        !status.wing_ready(),
        "one long-range aircraft does not supply a two-plane strike"
    );
}

#[test]
fn reserved_airfield_keeps_priority_after_status_refresh() {
    let (mut g, mut ai, plan, _, _) = fixture();
    assert!(ai.air_surge_production(&mut g, 0));
    assert_eq!(ai.air_surge_status.aerodromes_committed, 1);
    let cid = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| !g.cities[cid].queue.is_empty())
        .unwrap();
    let item = g.cities[&cid].queue[0].clone();
    assert!(matches!(item, Item::District { .. }));
    let value = ai.production_value(&g, 0, cid, &item, &plan, &ai.counts(&g, 0));
    assert!(value > 7000.0, "reserved airfield lost priority: {value}");
    ai.preempt_margin = 1.25;
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn final_queued_bomber_retains_priority_without_ordering_a_third() {
    let (mut g, mut ai, plan, first, second) = fixture();
    let bomber = Item::Unit {
        unit: name!("bomber"),
    };
    g.spawn_test_unit("bomber", 0, (6, 12));
    g.cities.get_mut(&first).unwrap().queue = vec![bomber.clone()];
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
    assert_eq!(ai.air_surge_status.bombers_committed, 2);
    let counts = ai.counts(&g, 0);
    let committed = ai.production_value(&g, 0, first, &bomber, &plan, &counts);
    let duplicate = ai.production_value(&g, 0, second, &bomber, &plan, &counts);
    assert!(
        committed > duplicate + 5000.0,
        "last bomber lost priority: {committed}"
    );
    assert!(
        duplicate < 7000.0,
        "third bomber gained package priority: {duplicate}"
    );
}

#[test]
fn second_queued_airfield_is_reserved_only_until_the_launch_wing_is_committed() {
    let (mut g, mut ai, _, first, second) = fixture();
    let field =
        |g: &Game, cid| {
            g.producible_items(0, cid).into_iter().find(|item|
        matches!(item, Item::District { district, .. } if district == "aerodrome")).unwrap()
        };
    let a = field(&g, first);
    let b = field(&g, second);
    g.cities.get_mut(&first).unwrap().queue = vec![a];
    g.cities.get_mut(&second).unwrap().queue = vec![b.clone()];
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(ai.air_surge_status.aerodromes_committed, 2);
    assert!(ai
        .air_surge_city_production_value(&g, 0, second, &b, 2.0)
        .is_some());
    let third = g.found_city_for(0, (6, 18), None);
    g.cities.get_mut(&third).unwrap().pop = 12;
    let c = field(&g, third);
    g.cities.get_mut(&third).unwrap().queue = vec![c.clone()];
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, third, &c, 2.0),
        None
    );
    g.cities.get_mut(&third).unwrap().queue.clear();
    g.spawn_test_unit("bomber", 0, (6, 12));
    g.spawn_test_unit("bomber", 0, (12, 12));
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, second, &b, 2.0),
        None
    );
    ai.air_surge_plan = None;
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, second, &b, 2.0),
        None
    );
}

#[test]
fn capture_body_quota_preserves_its_last_queue_but_releases_excess() {
    let (mut g, ai, _, first, second) = fixture();
    let body = Item::Unit {
        unit: name!("musketman"),
    };
    for pos in [(6, 12), (7, 12), (8, 12)] {
        g.spawn_test_unit("musketman", 0, pos);
    }
    g.cities.get_mut(&first).unwrap().queue = vec![body.clone()];
    assert!(ai
        .air_surge_city_production_value(&g, 0, first, &body, 2.0)
        .is_some());
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, second, &body, 2.0),
        None
    );
    g.spawn_test_unit("musketman", 0, (9, 12));
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, first, &body, 2.0),
        None
    );
}

/// Live King 20261001T000033Z: the first Aerodrome went to the first idle
/// city at 34 turns, and the first Bomber to a 26-turn city while another
/// built one in 7. A Domination wing's airfield goes to the fastest city that
/// can take it, switching that city's queue, and a much slower city is not
/// paid to take it instead.
#[test]
fn the_domination_airfield_goes_to_the_fastest_city_even_when_it_is_busy() {
    let (mut g, mut ai, _plan, first, second) = fixture();
    ai.victory_target = Some(crate::ai::advanced::VictoryTarget::Domination);
    // A mining town and a hamlet.
    let first_pos = g.cities[&first].pos;
    // Mined hills beyond the first ring; the first ring stays flat so the
    // Aerodrome (flat land only) has room.
    for pos in g.wdisk(first_pos, 3) {
        if g.wdist(pos, first_pos) < 2 {
            continue;
        }
        if let Some(tile) = g.map.tiles.get_mut(&pos) {
            tile.terrain = name!("plains");
            tile.hills = true;
            tile.improvement = Some(name!("mine"));
        }
    }
    g.cities.get_mut(&second).unwrap().pop = 1;
    g.cities.get_mut(&first).unwrap().queue = vec![Item::Unit { unit: name!("builder") }];
    g.cities.get_mut(&second).unwrap().queue.clear();
    let field = |g: &Game, cid: u32| {
        g.producible_items(0, cid).into_iter().find(|item| {
            matches!(item, Item::District { district, .. } if g.district_family(*district) == name!("aerodrome"))
        })
    };
    let slow = field(&g, second).expect("the hamlet could place one");
    let fast = field(&g, first).expect("the mining town could place one");
    let rate = |g: &Game, cid: u32, item: &Item| {
        g.item_remaining_cost_for_city(0, cid, item)
            / (g.city_yields(cid).production * g.item_prod_mult(0, cid, Some(item))).max(0.1)
    };
    assert!(
        rate(&g, second, &slow) > rate(&g, first, &fast) * AIR_SURGE_SLOW_FACTOR + AIR_SURGE_SLOW_SLACK,
        "precondition: {} turns against {}",
        rate(&g, second, &slow),
        rate(&g, first, &fast)
    );
    let turns = rate(&g, second, &slow);
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, second, &slow, turns),
        None,
        "the hamlet is not paid to raise the field"
    );

    assert!(ai.air_surge_production(&mut g, 0));
    assert!(matches!(
        g.cities[&first].queue.first(),
        Some(Item::District { district, .. }) if g.district_family(*district) == name!("aerodrome")
    ));
    assert!(g.cities[&second].queue.is_empty());

    // Outside the Domination lane the first idle city still takes it.
    let (mut g, mut ai, _plan, first, second) = fixture();
    ai.victory_target = None;
    g.cities.get_mut(&first).unwrap().queue = vec![Item::Unit { unit: name!("builder") }];
    g.cities.get_mut(&second).unwrap().queue.clear();
    assert!(ai.air_surge_production(&mut g, 0));
    assert!(matches!(
        g.cities[&second].queue.first(),
        Some(Item::District { district, .. }) if g.district_family(*district) == name!("aerodrome")
    ));
}

/// Live King 20261001T080758Z: Advanced Flight at turn 156 and one Aerodrome,
/// so Bogota trained the two launch Bombers one after the other (163, 168)
/// while cities within a fifth of its production trained nothing for the
/// wing. With `air-surge-2` a second airfield rises during the beeline and
/// trains half the launch wing beside the first.
#[test]
fn a_second_airfield_rises_during_the_beeline_for_half_the_launch_wing() {
    let run = |v2: bool| {
        let (mut g, mut ai, _, first, second) = fixture();
        // Two working cities of like production: mined hills on the first
        // ring, but for one flat tile left for an airfield.
        for cid in [first, second] {
            let centre = g.cities[&cid].pos;
            let mut ring = g.wdisk(centre, 1);
            ring.retain(|pos| *pos != centre);
            ring.sort();
            for pos in ring.into_iter().skip(1) {
                let tile = g.map.tiles.get_mut(&pos).unwrap();
                tile.hills = true;
                tile.improvement = Some(name!("mine"));
            }
        }
        g.players[0].techs.remove(&Name::new(AIR_SURGE_GOAL_TECH));
        crate::game::install_test_district(&mut g, first, "aerodrome");
        if !v2 {
            ai.disable_air_surge_2();
            ai.air_surge = true;
        }
        ai.air_surge_plan.as_mut().unwrap().phase = AirSurgePhase::Beeline;
        ai.air_surge_plan.as_mut().unwrap().tech_turn = None;
        ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
        assert_eq!(ai.air_surge_status.aerodromes_committed, 1);
        assert!(ai.air_surge_status.metal_ready);
        assert!(AdvancedAi::air_surge_research_eta(&g, 0) > 0.0);
        ai.air_surge_production(&mut g, 0);
        g.cities[&second].queue.first().cloned()
    };
    let field =
        |item: &Option<Item>| matches!(item, Some(Item::District { district, .. }) if district == "aerodrome");
    let v1 = run(false);
    assert!(!field(&v1), "one airfield is the whole v1 requirement: {v1:?}");
    let v2 = run(true);
    assert!(field(&v2), "the second airfield: {v2:?}");
}
