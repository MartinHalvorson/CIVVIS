use super::*;
use crate::game::HostMenuEntry;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32, Pos) {
    fixture_players(2)
}

fn fixture_players(players: usize) -> (Game, AdvancedAi, StrategicPlan, u32, u32, Pos) {
    let mut g = Game::new_full(players, 44, 26, 59012600, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.clear_mirror_cities();
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.owner_city = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
    }
    let home = g.found_city_for(0, (6, 10), None);
    let producer = g.found_city_for(0, (10, 10), None);
    let capital = g.found_city_for(1, (24, 10), None);
    if players > 2 {
        g.found_city_for(2, (34, 18), None);
    }
    // Encampments cannot be adjacent to their city centre. Give the
    // producer an observed second ring, rather than inventing an offer.
    for pos in g.wdisk((10, 10), 2) {
        if g.map.tiles[&pos].owner_city.is_none() {
            g.map.tiles.get_mut(&pos).unwrap().owner_city = Some(producer);
            g.cities.get_mut(&producer).unwrap().owned_tiles.push(pos);
        }
    }
    g.current = 0;
    g.turn = 170;
    g.at_war.insert((0, 1));
    g.record_contact(0, 1);
    g.players[0].techs.extend(g.rules.techs.keys().copied());
    g.players[0].gold = 1000.0;
    g.players[0].gold_per_turn = 60.0;
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.host_observed = Arc::new(g.map.tiles.keys().copied().collect());
    for cid in [home, producer] {
        g.cities.get_mut(&cid).unwrap().pop = 12;
        g.cities.get_mut(&cid).unwrap().queue.clear();
    }
    g.spawn_test_unit("bomber", 0, (6, 10));
    g.spawn_test_unit("bomber", 0, (10, 10));
    Arc::make_mut(&mut g.host_buildable).insert(home, BTreeMap::new());
    offer(&mut g, producer, "district:encampment", 2.0);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_forward_airfield_project();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(capital),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, producer, capital, (14, 10))
}

fn offer(g: &mut Game, city: u32, key: &str, turns: f64) {
    Arc::make_mut(&mut g.host_buildable).insert(
        city,
        BTreeMap::from([(
            key.to_string(),
            HostMenuEntry {
                cost: Some(100.0),
                turns: Some(turns),
            },
        )]),
    );
}

fn install_project(ai: &mut AdvancedAi, g: &Game, producer: u32, capital: u32, field: Pos) {
    ai.forward_airfield_project_state = Some(FieldProject {
        city: g.cities[&producer].pos,
        capital: g.cities[&capital].pos,
        field,
        started: g.turn,
    });
}

fn renumber_cities(g: &Game) -> Game {
    let mut next = g.clone();
    next.cities = g
        .cities
        .values()
        .cloned()
        .map(|mut city| {
            city.id += 1000;
            (city.id, city)
        })
        .collect();
    for tile in next.map.tiles.values_mut() {
        tile.owner_city = tile.owner_city.map(|id| id + 1000);
    }
    next.host_buildable = Arc::new(
        g.host_buildable
            .iter()
            .map(|(id, menu)| (id + 1000, menu.clone()))
            .collect(),
    );
    // Deserialization rebuilds the private city-position index, as a real
    // fresh board does. Host-only offer facts are deliberately not saved.
    let host_buildable = Arc::clone(&next.host_buildable);
    let host_observed = Arc::clone(&next.host_observed);
    let mut next: Game = serde_json::from_slice(&serde_json::to_vec(&next).unwrap()).unwrap();
    next.host_buildable = host_buildable;
    next.host_observed = host_observed;
    for city in next.cities.values() {
        assert_eq!(next.city_at(city.pos), Some(city.id));
    }
    next
}

#[test]
fn a_held_stage_follows_its_city_when_a_fresh_board_renumbers_both_cities() {
    let (mut g, mut ai, plan, producer, _, _) = fixture();
    ai.claim_forward_airfield_project(&mut g, 0, &plan);
    let item = g.cities[&producer].queue[0].clone();
    assert!(ai.forward_airfield_project_holds(&g, 0, producer, &plan, &item));
    let next = renumber_cities(&g);
    let mut next_plan = plan;
    next_plan.target_city = next_plan.target_city.map(|id| id + 1000);
    assert!(ai.forward_airfield_project_holds(&next, 0, producer + 1000, &next_plan, &item));
}

#[test]
fn a_fresh_board_cannot_restart_an_expired_field_project_by_renumbering_its_capital() {
    let (g, mut ai, mut plan, producer, capital, field) = fixture();
    install_project(&mut ai, &g, producer, capital, field);
    let started = g.turn;
    let mut next = renumber_cities(&g);
    next.turn += next.standard_duration(PROJECT_STANDARD_TURNS) + 1;
    plan.target_city = Some(capital + 1000);
    ai.claim_forward_airfield_project(&mut next, 0, &plan);
    assert!(next.cities[&(producer + 1000)].queue.is_empty());
    assert_eq!(
        ai.forward_airfield_project_state.as_ref().unwrap().started,
        started
    );
}

#[test]
fn one_idle_producer_starts_a_legal_encampment_and_keeps_it_through_both_governors() {
    let (mut g, mut ai, plan, producer, _, _) = fixture();
    assert!(
        g.producible_items(0, producer)
            .iter()
            .any(|i| matches!(i, Item::District { district, .. } if district == "encampment")),
        "actual initial offer"
    );
    ai.claim_forward_airfield_project(&mut g, 0, &plan);
    let item = g.cities[&producer].queue.first().cloned();
    assert!(
        matches!(item, Some(Item::District { district, .. }) if district == "encampment"),
        "one offered preparation stage"
    );
    let item = item.unwrap();
    assert_eq!(
        g.cities
            .values()
            .filter(|c| c.owner == 0 && !c.queue.is_empty())
            .count(),
        1
    );
    assert!(ai.forward_airfield_project_holds(&g, 0, producer, &plan, &item));
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(
        g.cities[&producer].queue.first(),
        Some(&item),
        "strategic rescoring keeps the accepted stage"
    );
    ai.delegated_cities(&mut g, 0, &plan);
    assert_eq!(
        g.cities[&producer].queue.first(),
        Some(&item),
        "delegated governor keeps the accepted stage"
    );
}

#[test]
fn a_completed_armory_advances_to_the_actual_engineer_offer() {
    let (mut g, mut ai, plan, producer, _, _) = fixture();
    g.cities
        .get_mut(&producer)
        .unwrap()
        .districts
        .insert(crate::name!("encampment"), (11, 10));
    let tile = g.map.tiles.get_mut(&(11, 10)).unwrap();
    tile.district = Some(crate::name!("encampment"));
    tile.owner_city = Some(producer);
    g.cities
        .get_mut(&producer)
        .unwrap()
        .buildings
        .extend([crate::name!("barracks"), crate::name!("armory")]);
    offer(&mut g, producer, "unit:military_engineer", 3.0);
    assert!(g.can_produce(
        0,
        producer,
        &Item::Unit {
            unit: crate::name!("military_engineer")
        }
    ));
    ai.claim_forward_airfield_project(&mut g, 0, &plan);
    assert_eq!(
        g.cities[&producer].queue.first(),
        Some(&Item::Unit {
            unit: crate::name!("military_engineer")
        })
    );
}

#[test]
fn an_engineer_builds_the_ordinary_legal_neutral_field_and_opens_its_slots() {
    let (mut g, mut ai, plan, producer, capital, field) = fixture();
    let engineer = g.spawn_test_unit("military_engineer", 0, field);
    install_project(&mut ai, &g, producer, capital, field);
    assert!(g
        .valid_improvements(0, field)
        .contains(&crate::name!("airstrip")));
    assert_eq!(
        ai.forward_airfield_engineer_step(&mut g, 0, engineer, &plan),
        Some(true)
    );
    assert_eq!(g.map.tiles[&field].improvement.as_deref(), Some("airstrip"));
    assert_eq!(g.air_capacity_at(0, field), 3);
    assert_eq!(g.units[&engineer].charges, 1);
}

#[test]
fn a_proposed_airstrip_cannot_restart_its_deadline_when_the_host_keeps_the_old_board() {
    let (mut prediction, mut ai, plan, producer, capital, field) = fixture();
    let engineer = prediction.spawn_test_unit("military_engineer", 0, field);
    install_project(&mut ai, &prediction, producer, capital, field);
    // The bridge's simulated success is a proposal. The next authoritative
    // board can still report the old plot and unspent Engineer charges.
    let mut host = prediction.clone();
    assert_eq!(
        ai.forward_airfield_engineer_step(&mut prediction, 0, engineer, &plan),
        Some(true)
    );
    assert!(host.map.tiles[&field].improvement.is_none());
    host.turn += host.standard_duration(PROJECT_STANDARD_TURNS) + 1;
    ai.claim_forward_airfield_project(&mut host, 0, &plan);
    assert_eq!(
        ai.forward_airfield_engineer_step(&mut host, 0, engineer, &plan),
        None,
        "an unconfirmed build cannot create a fresh attempt after the original deadline"
    );
    assert_eq!(host.units[&engineer].charges, 2);
}

#[test]
fn an_engineer_takes_a_legal_known_land_step_toward_its_project() {
    let (mut g, mut ai, plan, producer, capital, field) = fixture();
    let engineer = g.spawn_test_unit("military_engineer", 0, (10, 10));
    install_project(&mut ai, &g, producer, capital, field);
    let before = g.wdist(g.units[&engineer].pos, field);
    assert_eq!(
        ai.forward_airfield_engineer_step(&mut g, 0, engineer, &plan),
        Some(true)
    );
    assert!(g.wdist(g.units[&engineer].pos, field) < before);
}

#[test]
fn off_and_busy_queues_remain_unchanged() {
    let (mut g, mut ai, plan, producer, _, _) = fixture();
    ai.disable_forward_airfield_project();
    ai.claim_forward_airfield_project(&mut g, 0, &plan);
    assert!(g.cities[&producer].queue.is_empty());
    ai.enable_forward_airfield_project();
    let busy = Item::Unit {
        unit: crate::name!("settler"),
    };
    g.cities.get_mut(&producer).unwrap().queue = vec![busy.clone()];
    ai.claim_forward_airfield_project(&mut g, 0, &plan);
    assert_eq!(g.cities[&producer].queue.first(), Some(&busy));
}

#[test]
fn recovery_threats_peace_and_minor_capitals_cannot_start_a_project() {
    for case in 0..4 {
        let (mut g, mut ai, mut plan, producer, _, _) = fixture();
        match case {
            0 => plan.strategy = GrandStrategy::Recovery,
            1 => plan.threatened_city = Some(producer),
            2 => g.at_war.clear(),
            _ => g.players[1].is_minor = true,
        }
        ai.claim_forward_airfield_project(&mut g, 0, &plan);
        assert!(g.cities[&producer].queue.is_empty(), "case {case}");
    }
}

#[test]
fn nonfinite_or_insufficient_income_and_cash_cannot_reserve_the_chain() {
    for case in 0..5 {
        let (mut g, mut ai, plan, producer, _, _) = fixture();
        match case {
            0 => g.players[0].gold_per_turn = f64::NAN,
            1 => g.players[0].gold_per_turn = -3.0,
            2 => g.players[0].gold_per_turn = 6.0,
            3 => g.players[0].gold = 0.0,
            _ => g.players[0].gold = f64::INFINITY,
        }
        ai.claim_forward_airfield_project(&mut g, 0, &plan);
        assert!(g.cities[&producer].queue.is_empty(), "case {case}");
    }
}

#[test]
fn a_missing_or_slow_actual_offer_does_not_invent_a_stage() {
    for case in 0..3 {
        let (mut g, mut ai, plan, producer, _, _) = fixture();
        match case {
            0 => {
                Arc::make_mut(&mut g.host_buildable).insert(producer, BTreeMap::new());
            }
            1 => offer(&mut g, producer, "district:encampment", 100.0),
            _ => offer(&mut g, producer, "district:encampment", f64::NAN),
        }
        ai.claim_forward_airfield_project(&mut g, 0, &plan);
        assert!(g.cities[&producer].queue.is_empty(), "case {case}");
    }
}

#[test]
fn no_unseen_foreign_or_dangerous_site_is_claimed() {
    for case in 0..3 {
        let (mut g, mut ai, plan, producer, _, _) = fixture();
        for pos in g.map.tiles.keys().copied().collect::<Vec<_>>() {
            if g.wdist(pos, (24, 10)) <= 10 {
                match case {
                    0 => {
                        g.players[0].explored.remove(&pos);
                        Arc::make_mut(&mut g.host_observed).remove(&pos);
                    }
                    1 => {
                        Arc::make_mut(&mut g.blocked_city_sites).insert(pos);
                    }
                    _ => {
                        g.spawn_test_unit("modern_armor", 1, pos);
                    }
                }
            }
        }
        ai.claim_forward_airfield_project(&mut g, 0, &plan);
        assert!(g.cities[&producer].queue.is_empty(), "case {case}");
    }
}

#[test]
fn a_changed_target_and_an_expired_project_release_the_hold() {
    let (mut g, mut ai, mut plan, producer, capital, field) = fixture();
    install_project(&mut ai, &g, producer, capital, field);
    let item = Item::Building {
        building: crate::name!("armory"),
    };
    plan.target_player = None;
    assert!(!ai.forward_airfield_project_holds(&g, 0, producer, &plan, &item));
    plan.target_player = Some(1);
    g.turn += g.standard_duration(40) + 1;
    assert!(!ai.forward_airfield_project_holds(&g, 0, producer, &plan, &item));
}

#[test]
fn a_queued_forward_aerodrome_precedes_a_second_infrastructure_chain() {
    let (mut g, mut ai, plan, producer, _, field) = fixture();
    let other = g.found_city_for(0, (13, 12), None);
    g.cities.get_mut(&other).unwrap().pop = 12;
    g.map.tiles.get_mut(&field).unwrap().owner_city = Some(other);
    g.cities.get_mut(&other).unwrap().owned_tiles.push(field);
    offer(&mut g, other, "district:aerodrome", 2.0);
    let queued = Item::District {
        district: crate::name!("aerodrome"),
        pos: field,
    };
    assert!(
        g.can_produce(0, other, &queued),
        "the promised base is a legal build"
    );
    g.apply(
        0,
        &Action::Produce {
            city: other,
            item: queued,
        },
    )
    .unwrap();
    ai.claim_forward_airfield_project(&mut g, 0, &plan);
    assert!(g.cities[&producer].queue.is_empty());
    assert!(ai.forward_airfield_project_state.is_none());
}

#[test]
fn an_active_owned_field_already_in_range_prevents_a_duplicate_project() {
    let (mut g, mut ai, plan, producer, _, field) = fixture();
    let tile = g.map.tiles.get_mut(&field).unwrap();
    tile.improvement = Some(crate::name!("airstrip"));
    tile.improvement_owner = Some(0);
    assert_eq!(g.air_capacity_at(0, field), 3);
    ai.claim_forward_airfield_project(&mut g, 0, &plan);
    assert!(g.cities[&producer].queue.is_empty());
}

#[test]
fn an_expired_attempt_cannot_reset_its_budget_on_repeated_frames() {
    let (mut g, mut ai, plan, producer, capital, field) = fixture();
    install_project(&mut ai, &g, producer, capital, field);
    let started = g.turn;
    g.turn += g.standard_duration(40) + 1;
    for _ in 0..3 {
        ai.claim_forward_airfield_project(&mut g, 0, &plan);
    }
    assert!(g.cities[&producer].queue.is_empty());
    assert_eq!(
        ai.forward_airfield_project_state.as_ref().unwrap().started,
        started
    );
}

#[test]
fn each_offered_prerequisite_is_selected_before_an_unavailable_engineer() {
    for stage in ["barracks", "armory"] {
        let (mut g, mut ai, plan, producer, capital, field) = fixture();
        g.cities
            .get_mut(&producer)
            .unwrap()
            .districts
            .insert(crate::name!("encampment"), (11, 10));
        let tile = g.map.tiles.get_mut(&(11, 10)).unwrap();
        tile.district = Some(crate::name!("encampment"));
        tile.owner_city = Some(producer);
        if stage == "armory" {
            g.cities
                .get_mut(&producer)
                .unwrap()
                .buildings
                .push(crate::name!("barracks"));
        }
        offer(&mut g, producer, &format!("building:{stage}"), 2.0);
        install_project(&mut ai, &g, producer, capital, field);
        let expected = Item::Building {
            building: Name::new(stage),
        };
        assert!(
            g.can_produce(0, producer, &expected),
            "real prerequisite offer: {stage}"
        );
        ai.claim_forward_airfield_project(&mut g, 0, &plan);
        assert_eq!(g.cities[&producer].queue.first(), Some(&expected));
    }
}

#[test]
fn a_captured_original_major_capital_keeps_its_founders_class_after_elimination() {
    let (mut g, mut ai, mut plan, producer, capital, _) = fixture_players(3);
    g.cities.get_mut(&capital).unwrap().owner = 2;
    g.players[1].alive = false;
    g.at_war.insert((0, 2));
    g.record_contact(0, 2);
    plan.target_player = Some(2);
    assert_eq!(ai.forward_field_capital(&g, 0, &plan), Some(capital));
    ai.claim_forward_airfield_project(&mut g, 0, &plan);
    assert!(
        matches!(g.cities[&producer].queue.first(), Some(Item::District { district, .. }) if district == "encampment")
    );
}

#[test]
fn the_host_improvement_refusal_keeps_the_engineers_charge() {
    let (mut g, mut ai, plan, producer, capital, field) = fixture();
    let engineer = g.spawn_test_unit("military_engineer", 0, field);
    install_project(&mut ai, &g, producer, capital, field);
    Arc::make_mut(&mut g.blocked_improvements)
        .entry(field)
        .or_default()
        .insert(crate::name!("airstrip"));
    assert_eq!(
        ai.forward_airfield_engineer_step(&mut g, 0, engineer, &plan),
        None
    );
    assert_eq!(g.units[&engineer].charges, 2);
    assert!(g.map.tiles[&field].improvement.is_none());
}

#[test]
fn a_linked_engineer_and_an_unsafe_engineer_keep_the_ordinary_handler() {
    for unsafe_position in [false, true] {
        let (mut g, mut ai, plan, producer, capital, field) = fixture();
        let engineer = g.spawn_test_unit("military_engineer", 0, field);
        install_project(&mut ai, &g, producer, capital, field);
        if unsafe_position {
            g.spawn_test_unit("modern_armor", 1, (15, 10));
        } else {
            let guard = g.spawn_test_unit("infantry", 0, field);
            g.units.get_mut(&engineer).unwrap().linked_to = Some(guard);
            g.units.get_mut(&guard).unwrap().linked_to = Some(engineer);
        }
        assert_eq!(
            ai.forward_airfield_engineer_step(&mut g, 0, engineer, &plan),
            None
        );
        assert_eq!(g.units[&engineer].charges, 2);
        assert!(g.map.tiles[&field].improvement.is_none());
    }
}

/// The in-flight claim never starts a project; once one is in flight it
/// queues the next stage in the producer's idle queue.
#[test]
fn the_in_flight_claim_only_advances_a_chosen_project() {
    let (mut g, mut ai, plan, producer, capital, field) = fixture();
    ai.claim_forward_airfield_stage_in_flight(&mut g, 0, &plan);
    assert!(
        ai.forward_airfield_project_state.is_none(),
        "no project chosen"
    );
    assert!(g.cities[&producer].queue.is_empty());

    install_project(&mut ai, &g, producer, capital, field);
    ai.claim_forward_airfield_stage_in_flight(&mut g, 0, &plan);
    assert!(
        matches!(
            g.cities[&producer].queue.first(),
            Some(Item::District { district, .. }) if district == "encampment"
        ),
        "{:?}",
        g.cities[&producer].queue
    );

    // A project whose capital is no longer the objective waits for the late
    // claim, which resets it; nothing new is chosen early.
    let (mut g, mut ai, plan, producer, _, field) = fixture();
    ai.forward_airfield_project_state = Some(FieldProject {
        city: g.cities[&producer].pos,
        capital: (40, 20),
        field,
        started: g.turn,
    });
    ai.claim_forward_airfield_stage_in_flight(&mut g, 0, &plan);
    assert!(g.cities[&producer].queue.is_empty(), "stale capital");
    assert_eq!(
        ai.forward_airfield_project_state
            .as_ref()
            .map(|p| p.capital),
        Some((40, 20)),
        "left for the late claim to reset"
    );

    // Off, nothing moves even with a project held.
    let (mut g, mut ai, plan, producer, capital, field) = fixture();
    install_project(&mut ai, &g, producer, capital, field);
    ai.disable_forward_airfield_project();
    ai.forward_airfield_project_state = Some(FieldProject {
        city: g.cities[&producer].pos,
        capital: g.cities[&capital].pos,
        field,
        started: g.turn,
    });
    ai.claim_forward_airfield_stage_in_flight(&mut g, 0, &plan);
    assert!(g.cities[&producer].queue.is_empty(), "off");
}
