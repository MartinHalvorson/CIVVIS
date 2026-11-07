use super::*;
use crate::game::HostMenuEntry;
use std::sync::Arc;

fn fixture() -> (Game, AdvancedAi, Vec<u32>) {
    let (mut g, cities) =
        crate::ai::commercial_hub::tests::emperor_empire("culture-commerce-floor", 4737);
    let last = *cities.last().unwrap();
    crate::game::install_test_district(&mut g, last, "theater_square");
    g.cities
        .get_mut(&last)
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    let pos = g.cities[&last].pos;
    g.spawn_unit("trader", 0, pos);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.commercial_hub_in_the_strategic_queue = true;
    ai.base.commercial_hub_and_traders = true;
    (g, ai, cities)
}

fn unit(kind: &str) -> Item {
    Item::Unit {
        unit: crate::name::Name::new(kind),
    }
}

#[test]
fn idle_culture_opportunity_survives_routine_claim_and_reaches_governor() {
    let (mut g, mut ai, _) = fixture();
    let plan = ai.assess(&g, 0);
    let (cid, hub) = ai.culture_commerce_candidate(&g, 0, &plan).unwrap();
    assert!(matches!(&hub, Item::District { district, .. } if *district == "commercial_hub"));
    let mut full = g.clone();
    ai.run_idle_queue_claims(&mut full, 0, &plan);
    assert_eq!(full.cities[&cid].queue.first(), Some(&hub));
    g.cities.get_mut(&cid).unwrap().queue = vec![unit("builder")];
    ai.claim_culture_commerce(&mut g, 0, &plan, cid, hub.clone());
    assert_eq!(g.cities[&cid].queue.first(), Some(&hub));
    assert!(ai.base.commercial_hub_build_holds(&g, cid, &hub, None));
    assert!(AdvancedAi::production_commitment_is_legal(&g, 0, cid, &hub));
    assert_eq!(
        ai.culture_commerce_candidate(&g, 0, &plan),
        Some((cid, hub)),
        "finish the one chain already queued before reserving another Hub"
    );
}

#[test]
fn standing_hub_gets_market_then_open_capacity_gets_trader() {
    let (mut g, ai, cities) = fixture();
    let cid = cities[0];
    crate::game::install_test_district(&mut g, cid, "commercial_hub");
    let plan = ai.assess(&g, 0);
    let (chosen, market) = ai.culture_commerce_candidate(&g, 0, &plan).unwrap();
    assert_eq!(chosen, cid);
    assert_eq!(
        market,
        Item::Building {
            building: crate::name!("market")
        }
    );
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("market"));
    Arc::make_mut(&mut g.observed_trade_capacity).insert(0, 2);
    let (_, next) = ai.culture_commerce_candidate(&g, 0, &plan).unwrap();
    assert_eq!(next, unit("trader"));
    Arc::make_mut(&mut g.observed_trade_capacity).insert(0, 3);
    let (_, next) = ai.culture_commerce_candidate(&g, 0, &plan).unwrap();
    assert_eq!(
        next,
        unit("trader"),
        "fill the first three slots before stopping"
    );
    let pos = g.cities[&cid].pos;
    g.spawn_unit("trader", 0, pos);
    g.spawn_unit("trader", 0, pos);
    assert!(ai.culture_commerce_candidate(&g, 0, &plan).is_none());
}

#[test]
fn original_busy_queues_and_later_survival_claims_keep_their_place() {
    let (g, ai, cities) = fixture();
    let plan = ai.assess(&g, 0);
    for head in [
        unit("builder"),
        unit("settler"),
        unit("warrior"),
        Item::Building {
            building: crate::name!("amphitheater"),
        },
    ] {
        let mut busy = g.clone();
        for cid in &cities {
            busy.cities.get_mut(cid).unwrap().queue = vec![head.clone()];
        }
        assert!(ai.culture_commerce_candidate(&busy, 0, &plan).is_none());
    }
    let (cid, hub) = ai.culture_commerce_candidate(&g, 0, &plan).unwrap();
    for head in [
        unit("settler"),
        unit("warrior"),
        unit("trader"),
        Item::Building {
            building: crate::name!("walls"),
        },
        Item::Building {
            building: crate::name!("shrine"),
        },
    ] {
        let mut protected = g.clone();
        protected.cities.get_mut(&cid).unwrap().queue = vec![head.clone()];
        ai.claim_culture_commerce(&mut protected, 0, &plan, cid, hub.clone());
        assert_eq!(protected.cities[&cid].queue.first(), Some(&head));
    }
}

#[test]
fn only_eligible_culture_midgame_uses_the_floor() {
    let (g, ai, cities) = fixture();
    let plan = ai.assess(&g, 0);
    for mode in 0..7 {
        let mut game = g.clone();
        let mut agent = ai.clone();
        match mode {
            0 => agent.commercial_hub_in_the_strategic_queue = false,
            1 => agent.base.commercial_hub_and_traders = false,
            2 => {
                agent = AdvancedAi::targeting(VictoryTarget::Science);
                agent.commercial_hub_in_the_strategic_queue = true;
                agent.base.commercial_hub_and_traders = true;
            }
            3 => game.victory_conditions.culture = false,
            4 => agent.lane_lost = true,
            5 => game.turn = 20,
            _ => {
                for city in game.cities.values_mut() {
                    city.buildings.retain(|b| *b != "amphitheater");
                }
            }
        }
        assert!(
            agent.culture_commerce_candidate(&game, 0, &plan).is_none(),
            "mode {mode}"
        );
    }
    let mut attacked = g.clone();
    for cid in cities {
        attacked.cities.get_mut(&cid).unwrap().last_attacked = g.turn;
    }
    assert!(ai.culture_commerce_candidate(&attacked, 0, &plan).is_none());
}

#[test]
fn lighthouse_does_not_reserve_a_second_capacity_building_in_the_same_city() {
    let (mut g, ai, cities) = fixture();
    for cid in &cities {
        crate::game::install_test_district(&mut g, *cid, "commercial_hub");
        g.cities
            .get_mut(cid)
            .unwrap()
            .buildings
            .push(crate::name!("lighthouse"));
    }
    let plan = ai.assess(&g, 0);
    assert!(ai.culture_commerce_candidate(&g, 0, &plan).is_none());
}

fn interrupted_hub() -> (Game, AdvancedAi, u32, Item) {
    let (mut g, ai, _) = fixture();
    let plan = ai.assess(&g, 0);
    let (cid, hub) = ai.culture_commerce_candidate(&g, 0, &plan).unwrap();
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: hub.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&cid).unwrap().production = 14.0;
    // The urgent unit retains its busy queue. Its completion leaves the
    // original foundation and its banked work available for resumption.
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: unit("warrior"),
        },
    )
    .unwrap();
    Arc::make_mut(&mut g.host_buildable).insert(
        cid,
        [(
            Game::production_block_key(&hub),
            HostMenuEntry {
                cost: Some(92.0),
                turns: Some(15.0),
            },
        )]
        .into_iter()
        .collect(),
    );
    (g, ai, cid, hub)
}

#[test]
fn income_foundation_resumes_after_the_urgent_unit_finishes() {
    let (mut g, mut ai, cid, hub) = interrupted_hub();
    let plan = ai.assess(&g, 0);
    assert!(
        ai.culture_commerce_candidate(&g, 0, &plan)
            .is_none_or(|(chosen, _)| chosen != cid),
        "the busy military queue keeps priority"
    );
    g.cities.get_mut(&cid).unwrap().queue.clear();
    assert!(g.can_produce(0, cid, &hub));
    assert_eq!(
        ai.culture_commerce_candidate(&g, 0, &plan),
        Some((cid, hub.clone()))
    );
    ai.run_idle_queue_claims(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&hub));
    assert!(ai.base.commercial_hub_build_holds(&g, cid, &hub, None));
    assert_eq!(g.item_invested_production(cid, &hub), 14.0);
}

#[test]
fn paused_income_requires_a_bounded_legal_host_quote_and_safe_city() {
    let (mut g, ai, cid, hub) = interrupted_hub();
    g.cities.get_mut(&cid).unwrap().queue.clear();
    let plan = ai.assess(&g, 0);
    for mode in 0..4 {
        let mut game = g.clone();
        let mut guarded = plan.clone();
        match mode {
            0 => {
                Arc::make_mut(&mut game.host_buildable).remove(&cid);
            }
            1 => {
                Arc::make_mut(&mut game.host_buildable)
                    .get_mut(&cid)
                    .unwrap()
                    .get_mut(&Game::production_block_key(&hub))
                    .unwrap()
                    .turns = Some(21.0);
            }
            2 => {
                Arc::make_mut(&mut game.blocked_production)
                    .entry(cid)
                    .or_default()
                    .insert(Game::production_block_key(&hub));
            }
            _ => guarded.threatened_city = Some(cid),
        }
        assert!(
            ai.culture_commerce_candidate(&game, 0, &guarded)
                .is_none_or(|(chosen, _)| chosen != cid),
            "guard {mode}"
        );
    }
}

fn income_and_followup_slots() -> (Game, AdvancedAi, u32, Item, StrategicPlan) {
    let (mut g, mut ai, cities) = fixture();
    let cid = cities[0];
    g.players[0].civics.insert(crate::name!("drama_poetry"));
    g.cities.get_mut(&cid).unwrap().pop = 7;
    crate::game::install_test_district(&mut g, cid, "theater_square");
    let plan = ai.assess(&g, 0);
    let (chosen, hub) = ai.culture_commerce_candidate(&g, 0, &plan).unwrap();
    assert_eq!(chosen, cid);
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: hub.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&cid).unwrap().production = 16.0;
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            production: 8.0,
            ..Default::default()
        },
    );
    for other in cities.into_iter().filter(|other| *other != cid) {
        g.cities.get_mut(&other).unwrap().queue = vec![unit("builder")];
    }
    ai.enable_culture_seat_theater_first();
    (g, ai, cid, hub, plan)
}

#[test]
fn committed_income_finishes_before_a_new_routine_amphitheater_claim() {
    let (mut g, mut ai, cid, hub, plan) = income_and_followup_slots();
    let amphitheater = Item::Building {
        building: crate::name!("amphitheater"),
    };
    let mut control = g.clone();
    ai.claim_culture_theaters(&mut control, 0, &plan);
    assert_eq!(
        control.cities[&cid].queue.first(),
        Some(&amphitheater),
        "the real Culture claim reproduces the native interruption"
    );
    assert!(!AdvancedAi::live_gp_cultural_building_commitment(
        &control,
        0,
        cid,
        &amphitheater
    ));
    ai.run_idle_queue_claims(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&hub));
    assert_eq!(g.item_invested_production(cid, &hub), 16.0);
}

#[test]
fn a_waiting_writer_still_gets_the_new_activation_slots() {
    let (mut g, mut ai, cid, _, plan) = income_and_followup_slots();
    g.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "writer".into(),
            required_district: Some("theater_square".into()),
            ..Default::default()
        },
    );
    ai.run_idle_queue_claims(&mut g, 0, &plan);
    let amphitheater = Item::Building {
        building: crate::name!("amphitheater"),
    };
    assert_eq!(g.cities[&cid].queue.first(), Some(&amphitheater));
    assert!(AdvancedAi::live_gp_cultural_building_commitment(
        &g,
        0,
        cid,
        &amphitheater
    ));
}
