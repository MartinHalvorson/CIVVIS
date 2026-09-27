use super::super::*;
use crate::ai::Ai;
use crate::setup::GameSpeed;

fn fixture(stock: f64) -> (Game, AdvancedAi, Vec<u32>, [u32; 2]) {
    let mut g = Game::new_full(2, 40, 24, 380_200, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let first = g.found_city_for(0, (6, 12), None);
    let second = g.found_city_for(0, (12, 12), None);
    let objective = g.found_city_for(1, (24, 12), None);
    for goal in ["advanced_flight", "synthetic_materials"] {
        g.players[0].techs.extend(
            g.rules.tech_ancestors[goal]
                .iter()
                .map(|tech| Name::new(tech)),
        );
        g.players[0].techs.insert(Name::new(goal));
    }
    g.map.tiles.get_mut(&(6, 12)).unwrap().resource = Some(crate::name!("aluminum"));
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), stock);
    g.players[0].gold = 2000.0;
    g.players[0].gold_per_turn = 50.0;
    g.game_speed = GameSpeed::Online;
    g.at_war.clear();
    g.record_contact(0, 1);
    g.current = 0;
    g.turn = 150;
    for cid in [first, second] {
        crate::game::install_test_district(&mut g, cid, "aerodrome");
        g.cities.get_mut(&cid).unwrap().pop = 8;
        for pos in g.cities[&cid].owned_tiles.clone() {
            if pos != g.cities[&cid].pos {
                let tile = g.map.tiles.get_mut(&pos).unwrap();
                tile.hills = true;
                tile.improvement = Some(crate::name!("mine"));
            }
        }
    }
    let cavalry = [(6, 12), (6, 13), (12, 12), (12, 13)]
        .into_iter()
        .map(|pos| g.spawn_test_unit("cavalry", 0, pos))
        .collect();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.base.book_pos = 4;
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(objective),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    });
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 2.0);
    assert!(ai.threatened_city(&g, 0).is_none());
    assert!(ai.domination_bomber_queue_needed(&g, 0, first));
    (g, ai, cavalry, [first, second])
}

fn next_owned_turn(g: &mut Game) {
    loop {
        g.apply(g.current, &Action::EndTurn).unwrap();
        if g.current == 0 {
            break;
        }
    }
}

#[test]
fn stockpile_grace_pays_existing_ground_deficit_for_real_resource_ticks() {
    let (mut g, _, cavalry, cities) = fixture(34.0);
    for uid in cavalry {
        g.apply(0, &Action::UpgradeUnit { unit: uid }).unwrap();
    }
    assert_eq!(g.strategic_stockpile(0, crate::name!("aluminum")), 30.0);
    // A two-plane wing plus four Helicopters burns four Aluminum per turn.
    // Prove the short bank fails the actual upkeep processor before asking
    // the AI to reject its apparent speed-scaled grace-period bridge.
    let mut launched = g.clone();
    for cid in cities {
        launched.spawn_test_unit("bomber", 0, launched.cities[&cid].pos);
    }
    let grace = g.standard_duration(air_surge::AIR_SURGE_ALUMINUM_GRACE);
    for _ in 0..grace {
        next_owned_turn(&mut launched);
    }
    assert!(launched.players[0].strategic_resource_shortages[&crate::name!("aluminum")] > 0);
    assert!(
        AdvancedAi::air_surge_bomber_goal(&g, 0) < 2,
        "the launch bank must cover existing ground fuel deficits too"
    );
}

#[test]
fn actual_turn_keeps_the_aluminum_source_for_two_bomber_queues() {
    assert_dispatch_wing(false);
}

#[test]
fn observed_dispatch_keeps_the_aluminum_source_for_two_bomber_queues() {
    assert_dispatch_wing(true);
}

fn assert_dispatch_wing(observed: bool) {
    let (mut g, mut ai, cavalry, cities) = fixture(8.0);
    ai.observed_player = observed;
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
    let log_start = g.log.len();
    ai.take_turn(&mut g, 0);
    let bombers = g
        .units
        .values()
        .filter(|unit| unit.owner == 0 && g.rules.units[unit.kind].promotion_class == "air_bomber")
        .count();
    let queued = g.player_city_ids(0).iter().flat_map(|cid| &g.cities[cid].queue)
        .filter(|item| matches!(item, Item::Unit { unit } if g.rules.units[unit].promotion_class == "air_bomber"))
        .count();
    assert!(
        bombers + queued >= 2,
        "the canonical dispatcher must commit the launch wing"
    );
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
    assert!(cavalry.iter().all(|uid| g.units[uid].kind == "cavalry"));
    assert!(!g
        .log
        .since(log_start)
        .any(|(_, action)| matches!(action, Action::UpgradeUnit { .. })));
    // Complete the remaining real production order, with the engine placing
    // the aircraft and paying its already committed construction material.
    for cid in cities {
        if let Some(item) = g.cities[&cid].queue.first().cloned() {
            if matches!(item, Item::Unit { unit } if g.rules.units[unit].promotion_class == "air_bomber")
            {
                g.cities.get_mut(&cid).unwrap().production = g.item_cost_for_city(0, cid, &item);
            }
        }
    }
    next_owned_turn(&mut g);
    assert_eq!(
        g.units
            .values()
            .filter(
                |unit| unit.owner == 0 && g.rules.units[unit.kind].promotion_class == "air_bomber"
            )
            .count(),
        2
    );
    for _ in 0..g.standard_duration(air_surge::AIR_SURGE_ALUMINUM_GRACE) {
        // Even a later full modernization opportunity must leave fuel to fly.
        ai.upgrade_units_preserving_air_wing(&mut g, 0);
        next_owned_turn(&mut g);
        assert!(!g.players[0]
            .strategic_resource_shortages
            .contains_key(&crate::name!("aluminum")));
    }
}

#[test]
fn surplus_source_funds_only_the_ground_upgrades_it_can_sustain() {
    let (mut g, ai, cavalry, cities) = fixture(8.0);
    g.map
        .tiles
        .get_mut(&g.cities[&cities[1]].pos)
        .unwrap()
        .resource = Some(crate::name!("aluminum"));
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 4.0);
    ai.upgrade_units_preserving_air_wing(&mut g, 0);
    assert_eq!(
        cavalry
            .iter()
            .filter(|uid| g.units[uid].kind == "helicopter")
            .count(),
        2
    );
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
}

#[test]
fn inactive_lanes_and_immediate_defense_keep_the_shared_upgrade_actions() {
    for case in 0..6 {
        let (mut g, mut ai, _, cities) = fixture(8.0);
        match case {
            0 => ai.disable_air_surge_2(),
            1 => ai.retarget(VictoryTarget::Science),
            2 => {
                for cid in cities {
                    g.cities.get_mut(&cid).unwrap().districts.clear();
                }
            }
            3 => {
                g.players[0].techs.remove(&crate::name!("advanced_flight"));
            }
            4 => g.max_turns = g.turn + 1,
            5 => {
                g.at_war.insert((0, 1));
                g.spawn_test_unit("modern_armor", 1, (7, 12));
                g.spawn_test_unit("modern_armor", 1, (6, 11));
                assert!(ai.threatened_city(&g, 0).is_some());
            }
            _ => unreachable!(),
        }
        let mut control = g.clone();
        BasicAi::upgrade_units(&mut control, 0);
        ai.upgrade_units_preserving_air_wing(&mut g, 0);
        assert_eq!(
            g.log.iter().collect::<Vec<_>>(),
            control.log.iter().collect::<Vec<_>>(),
            "case {case}"
        );
        assert_eq!(g.players[0].gold, control.players[0].gold);
        assert_eq!(
            g.players[0].strategic_resources,
            control.players[0].strategic_resources
        );
    }
}

#[test]
fn free_ground_upkeep_does_not_take_the_wing_reserve() {
    let (mut g, ai, cavalry, _) = fixture(8.0);
    for uid in &cavalry {
        g.units.get_mut(uid).unwrap().free_upkeep = true;
    }
    ai.upgrade_units_preserving_air_wing(&mut g, 0);
    assert!(cavalry.iter().all(|uid| g.units[uid].kind == "helicopter"));
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
}

#[test]
fn pending_ground_queue_counts_against_the_next_upgrade() {
    let (mut g, ai, cavalry, cities) = fixture(8.0);
    g.map
        .tiles
        .get_mut(&g.cities[&cities[1]].pos)
        .unwrap()
        .resource = Some(crate::name!("aluminum"));
    g.apply(
        0,
        &Action::Produce {
            city: cities[0],
            item: Item::Unit {
                unit: crate::name!("helicopter"),
            },
        },
    )
    .unwrap();
    ai.upgrade_units_preserving_air_wing(&mut g, 0);
    assert_eq!(
        cavalry
            .iter()
            .filter(|uid| g.units[uid].kind == "helicopter")
            .count(),
        1
    );
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
}

#[test]
fn jet_bombers_fulfill_the_wing_instead_of_being_counted_as_ground_demand() {
    let (mut g, ai, cavalry, cities) = fixture(8.0);
    for cid in cities {
        g.spawn_test_unit("jet_bomber", 0, g.cities[&cid].pos);
    }
    ai.upgrade_units_preserving_air_wing(&mut g, 0);
    assert!(cavalry.iter().all(|uid| g.units[uid].kind == "cavalry"));
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
    next_owned_turn(&mut g);
    assert!(!g.players[0]
        .strategic_resource_shortages
        .contains_key(&crate::name!("aluminum")));
}

#[test]
fn appointed_package_rechecks_fuel_after_each_upgrade() {
    let (mut g, mut ai, cavalry, cities) = fixture(8.0);
    g.map
        .tiles
        .get_mut(&g.cities[&cities[1]].pos)
        .unwrap()
        .resource = Some(crate::name!("aluminum"));
    ai.war_plan = Some(WarPlan {
        target_player: 1,
        objective_city: g.player_city_ids(1)[0],
        breakthrough_tech: crate::name!("synthetic_materials"),
        assault_unit: crate::name!("helicopter"),
        predecessor: Some(crate::name!("cavalry")),
        breach_unit: None,
        estimated_research_turns: 0,
        estimated_production_turns: 0,
        estimated_upgrade_gold: 0.0,
        estimated_march_turns: 3,
        phase: WarPhase::Mobilize,
        appointed_turn: g.turn,
        tech_turn: Some(g.turn),
        declared_turn: None,
        last_reviewed_turn: g.turn,
        recovery_assessments: 0,
    });
    ai.execute_war_upgrades(&mut g, 0);
    assert_eq!(
        cavalry
            .iter()
            .filter(|uid| g.units[uid].kind == "helicopter")
            .count(),
        2
    );
}

#[test]
fn production_and_cash_cannot_replace_the_reserved_fuel_with_a_ground_unit() {
    let (mut g, ai, cavalry, cities) = fixture(8.0);
    for uid in cavalry {
        g.remove_unit(uid);
    }
    // A four-city offensive with no land army clears the real purchase
    // scorer's need floor; the control must actually want this alternative.
    g.found_city_for(0, (6, 20), None);
    g.found_city_for(0, (12, 20), None);
    g.cities
        .get_mut(&cities[0])
        .unwrap()
        .production_progress
        .insert("unit:helicopter".into(), 299.0);
    let plan = ai.plan.as_ref().unwrap().clone();
    let counts = ai.counts(&g, 0);
    let item = Item::Unit {
        unit: crate::name!("helicopter"),
    };
    let mut control_ai = ai.clone();
    control_ai.disable_air_surge_2();
    let score = control_ai.production_value(&g, 0, cities[0], &item, &plan, &counts);
    assert!(
        score > 120.0,
        "control score {score}; military={}; alternatives={:?}",
        counts.military,
        g.producible_items(0, cities[0])
            .into_iter()
            .filter(|item| matches!(item, Item::Unit { .. }))
            .collect::<Vec<_>>()
    );
    assert!(ai.production_value(&g, 0, cities[0], &item, &plan, &counts) < -1000.0);
    let purchase = Action::Buy {
        city: cities[0],
        unit: crate::name!("helicopter"),
        formation: 0,
        currency: "gold".into(),
    };
    let context = PurchaseScoreContext {
        g: &g,
        pid: 0,
        plan: &plan,
        counts: &counts,
        bank: g.players[0].gold,
        reserve: 125.0,
    };
    assert!(control_ai
        .gold_purchase_score(context, &purchase, cities[0], &item)
        .is_some());
    assert!(ai
        .gold_purchase_score(context, &purchase, cities[0], &item)
        .is_none());
    let mut control = g.clone();
    control.apply(0, &purchase).unwrap();
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&control, 0), 1);
}

#[test]
fn the_bank_pays_training_once_then_carries_two_real_aircraft() {
    let (mut g, mut ai, _, cities) = fixture(30.0);
    g.map.tiles.get_mut(&(6, 12)).unwrap().resource = None;
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 0.0);
    assert!(ai.air_surge_production(&mut g, 0));
    assert!(ai.air_surge_production(&mut g, 0));
    assert_eq!(g.strategic_stockpile(0, crate::name!("aluminum")), 28.0);
    for cid in cities {
        let item = g.cities[&cid].queue[0].clone();
        g.cities.get_mut(&cid).unwrap().production = g.item_cost_for_city(0, cid, &item);
    }
    next_owned_turn(&mut g);
    assert_eq!(
        g.units
            .values()
            .filter(|unit| unit.owner == 0 && unit.kind == "bomber")
            .count(),
        2
    );
    for _ in 0..g.standard_duration(air_surge::AIR_SURGE_ALUMINUM_GRACE) {
        next_owned_turn(&mut g);
        assert!(!g.players[0]
            .strategic_resource_shortages
            .contains_key(&crate::name!("aluminum")));
    }
}

#[test]
fn an_oil_upgrade_uses_its_own_supply() {
    let (mut g, ai, cavalry, _) = fixture(8.0);
    g.remove_unit(cavalry[0]);
    let tank = g.spawn_test_unit("tank", 0, (6, 12));
    g.players[0].techs.insert(crate::name!("composites"));
    g.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 20.0);
    ai.upgrade_units_preserving_air_wing(&mut g, 0);
    assert_eq!(g.units[&tank].kind, "modern_armor");
    assert_eq!(g.strategic_stockpile(0, crate::name!("aluminum")), 8.0);
}

#[test]
fn launch_appointment_uses_capture_cavalry_that_does_not_displace_its_wing() {
    let (mut g, mut ai, _, cities) = fixture(8.0);
    let objective = g.found_city_for(1, (20, 12), None);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    produce_wing(&mut g, cities);
    ai.maintain_air_surge(&g, 0);
    // Appointment starts in Beeline; the next review applies actual readiness.
    ai.maintain_air_surge(&g, 0);
    let plan = ai
        .air_surge_plan
        .as_ref()
        .expect("an in-range supplied objective");
    assert_eq!(plan.objective_city, objective);
    assert!(
        matches!(
            plan.phase,
            air_surge::AirSurgePhase::Strike | air_surge::AirSurgePhase::Exploit
        ),
        "two real aircraft and four capture Cavalry must release the launch; body={}, phase={:?}",
        plan.body_unit,
        plan.phase
    );
    assert_eq!(ai.air_surge_launch_estimate(&g, 0), (0, 0));
}

fn produce_wing(g: &mut Game, cities: [u32; 2]) {
    for cid in cities {
        let item = Item::Unit {
            unit: crate::name!("bomber"),
        };
        g.apply(
            0,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .unwrap();
        g.cities.get_mut(&cid).unwrap().production = g.item_cost_for_city(0, cid, &item);
    }
    next_owned_turn(g);
    assert_eq!(
        g.units
            .values()
            .filter(|unit| unit.owner == 0 && unit.kind == "bomber")
            .count(),
        2
    );
}

#[test]
fn an_existing_helicopter_appointment_releases_the_reserved_wing() {
    let (mut g, mut ai, _, cities) = fixture(8.0);
    g.found_city_for(1, (20, 12), None);
    ai.air_surge_2 = false;
    ai.air_surge = false;
    let appointed = ai.choose_air_surge(&g, 0).unwrap();
    assert_eq!(appointed.body_unit, "helicopter");
    ai.air_surge_plan = Some(appointed);
    ai.enable_air_surge_2();
    produce_wing(&mut g, cities);
    ai.maintain_air_surge(&g, 0);
    let plan = ai.air_surge_plan.as_ref().unwrap();
    assert_eq!(plan.body_unit, "cavalry");
    assert_eq!(plan.phase, air_surge::AirSurgePhase::Strike);
}

#[test]
fn escort_choice_keeps_ready_helicopters_and_inactive_controls() {
    for case in ["ready_helicopters", "surplus", "off", "science", "wounded"] {
        let (mut g, mut ai, cavalry, _) = fixture(8.0);
        match case {
            "ready_helicopters" => {
                g.map.tiles.get_mut(&(12, 12)).unwrap().resource = Some(crate::name!("aluminum"));
                for uid in cavalry.iter().take(2) {
                    g.apply(0, &Action::UpgradeUnit { unit: *uid }).unwrap();
                }
                assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
            }
            "surplus" => {
                g.map.tiles.get_mut(&(12, 12)).unwrap().resource = Some(crate::name!("aluminum"));
            }
            "off" => {
                ai.air_surge_2 = false;
                ai.air_surge = false;
            }
            "science" => {
                ai = AdvancedAi::targeting(VictoryTarget::Science);
                ai.enable_air_surge_2();
            }
            "wounded" => {
                for uid in cavalry {
                    g.units.get_mut(&uid).unwrap().hp = 49;
                }
            }
            _ => unreachable!(),
        }
        assert_eq!(
            ai.air_surge_body_preserving_wing(&g, 0),
            AdvancedAi::air_surge_body(&g, 0),
            "{case}"
        );
    }
}

#[test]
fn retained_cavalry_spots_then_captures_with_the_physically_produced_wing() {
    let (mut g, mut ai, cavalry, cities) = fixture(8.0);
    let objective = g.found_city_for(1, (20, 12), None);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    produce_wing(&mut g, cities);
    let first_plane = g
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "bomber")
        .min_by_key(|unit| g.wdist(unit.pos, (6, 12)))
        .unwrap()
        .id;
    g.apply(
        0,
        &Action::AirRebase {
            unit: first_plane,
            to: (12, 12),
        },
    )
    .unwrap();
    next_owned_turn(&mut g);
    g.apply(
        0,
        &Action::MoveTo {
            unit: cavalry[2],
            to: (16, 12),
        },
    )
    .unwrap();
    next_owned_turn(&mut g);
    ai.maintain_air_surge(&g, 0);
    ai.maintain_air_surge(&g, 0);
    assert_eq!(
        ai.air_surge_plan.as_ref().unwrap().phase,
        air_surge::AirSurgePhase::Strike
    );
    g.players[0].denounced_until.insert(1, g.turn + 25);
    g.players[0].denounced_since.insert(1, g.turn - 5);
    assert!(ai.air_surge_opening(&mut g, 0, 1));
    assert!(g.is_at_war(0, 1));
    assert!(cavalry.iter().all(|uid| g.units[uid].kind == "cavalry"));
    assert!(!g.player_can_see(0, (20, 12)));
    let mut ground_plan = ai.plan.clone().unwrap();
    ground_plan.target_city = Some(objective);
    g.cities.get_mut(&objective).unwrap().hp = 30;
    let before = g.log.len();
    let reserved = ai.plan_air_city_assault(&mut g, 0, &ground_plan);
    assert!(reserved.contains(&cavalry[2]));
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
    assert!(
        first_bomb > 0,
        "cavalry must establish sight before the volley"
    );
    assert_eq!(g.cities[&objective].owner, 0);
    assert_eq!(g.units[&cavalry[2]].pos, (20, 12));
    assert!(ai.planned_air_city_assault().unwrap().moved_to_spot);
    assert!(g.legal_city_disposition_actions(0).is_empty());
}
