use super::*;
use crate::name;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(2, 40, 24, 372700, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.hills = true;
        tile.feature = None;
        tile.resource = None;
    }
    let city = g.found_city_for(0, (8, 8), None);
    g.found_city_for(1, (28, 14), None);
    g.cities.get_mut(&city).unwrap().pop = 4;
    g.cities.get_mut(&city).unwrap().queue.clear();
    g.players[0].techs.insert(name!("writing"));
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 10.0;
    g.record_contact(0, 1);
    g.turn = 40;
    g.current = 0;
    std::sync::Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        1,
        crate::rules::Yields {
            science: 100.0,
            ..Default::default()
        },
    );
    let mut ai = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    ai.enable_research_building_catchup();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 40,
        rush: false,
    };
    (g, ai, plan, city)
}

#[test]
fn research_shortfall_can_start_a_missing_campus_in_a_mature_domination_city() {
    for strategy in [GrandStrategy::Expansion, GrandStrategy::Conquest] {
        let (mut g, ai, mut plan, city) = fixture();
        plan.strategy = strategy;
        let (chosen, item, debt) = ai
            .higher_level_investment_target(&g, 0, &plan)
            .expect("a missing research foundation must be actionable");
        assert_eq!(chosen, city);
        assert_eq!(debt, Debt::Research);
        assert!(
            matches!(&item, Item::District { district, .. } if g.district_family(*district) == "campus")
        );
        ai.reserve_higher_level_investment(&mut g, 0, &plan);
        assert_eq!(g.cities[&city].queue.first(), Some(&item));
        assert!(AdvancedAi::placed_campus_research_item(&g, &item));
        assert!(
            AdvancedAi::campus_research_building(&g, &item),
            "the ordinary commitment guard must protect the new foundation"
        );
    }
}

#[test]
fn a_queued_foundation_does_not_start_another_or_block_an_available_library() {
    let (mut g, ai, plan, city) = fixture();
    let second = g.found_city_for(0, (18, 8), None);
    g.cities.get_mut(&second).unwrap().pop = 4;
    g.cities.get_mut(&second).unwrap().queue.clear();
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    let queued = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| !g.cities[cid].queue.is_empty())
        .expect("one research foundation queued");
    assert!(
        ai.higher_level_investment_target(&g, 0, &plan).is_none(),
        "one new campus commitment at a time"
    );
    let idle = if queued == city { second } else { city };
    let item = g.producible_items(0, idle).into_iter().find(|i| matches!(i, Item::District { district, .. } if g.district_family(*district) == "campus")).unwrap();
    let Item::District { district, pos } = item else {
        unreachable!()
    };
    g.cities
        .get_mut(&idle)
        .unwrap()
        .districts
        .insert(district, pos);
    g.map.tiles.get_mut(&pos).unwrap().district = Some(district);
    // Direct fixture completion bypassed the action layer; discard the old
    // production catalog before checking the completed district's menu.
    g = g.speculative_clone();
    assert!(g.producible_items(0, idle).contains(&Item::Building {
        building: name!("library"),
    }));
    let (chosen, item, _) = ai
        .higher_level_investment_target(&g, 0, &plan)
        .expect("library remains eligible while the other campus builds");
    assert_eq!(chosen, idle);
    assert_eq!(
        item,
        Item::Building {
            building: name!("library")
        }
    );
}

#[test]
fn missing_campus_reservation_respects_growth_lane_recovery_and_existing_work() {
    for case in [
        "small", "tech", "lane", "parity", "recovery", "threat", "queue", "clock",
    ] {
        let (mut g, mut ai, mut plan, city) = fixture();
        match case {
            "small" => g.cities.get_mut(&city).unwrap().pop = 3,
            "tech" => {
                g.players[0].techs.remove(&name!("writing"));
            }
            "lane" => {
                ai = AdvancedAi::targeting(super::super::VictoryTarget::Science);
                ai.enable_research_building_catchup();
            }
            "parity" => {
                std::sync::Arc::make_mut(&mut g.observed_yield_adjustments).clear();
            }
            "recovery" => plan.strategy = GrandStrategy::Recovery,
            "threat" => plan.threatened_city = Some(city),
            "queue" => g.cities.get_mut(&city).unwrap().queue.push(Item::Unit {
                unit: name!("warrior"),
            }),
            "clock" => g.turn = 249,
            _ => unreachable!(),
        }
        assert!(
            ai.higher_level_investment_target(&g, 0, &plan).is_none(),
            "{case}"
        );
    }
}
