use super::*;
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
fn original_busy_queues_and_later_survival_or_culture_claims_keep_their_place() {
    let (g, ai, cities) = fixture();
    let plan = ai.assess(&g, 0);
    for kind in ["builder", "settler", "warrior"] {
        let mut busy = g.clone();
        for cid in &cities {
            busy.cities.get_mut(cid).unwrap().queue = vec![unit(kind)];
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
        Item::Building {
            building: crate::name!("amphitheater"),
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
