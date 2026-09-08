use super::*;
use crate::name::Name;
use crate::setup::GameSpeed;

fn board() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new(2, 32, 22, 71, 250, 0);
    g.game_speed = GameSpeed::Online;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let cid = g.found_city_for(0, g.units[&settler].pos, None);
    g.remove_unit(settler);
    g.players[0].gold_per_turn = 10.0;
    g.players[0].gold = 500.0;
    g.cities.get_mut(&cid).unwrap().queue.clear();
    g.cities.get_mut(&cid).unwrap().buildings.clear();
    g.turn = 40;
    g.record_contact(0, 1);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 5,
        assessed_turn: 40,
        rush: false,
    };
    (g, AdvancedAi::new(), plan, cid)
}

fn rival_yield(g: &mut Game, culture: f64, science: f64) {
    std::sync::Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        1,
        crate::rules::Yields {
            culture,
            science,
            ..Default::default()
        },
    );
}

fn district(g: &mut Game, cid: u32, kind: &str) {
    let pos = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|p| *p != g.cities[&cid].pos)
        .unwrap();
    g.map.tiles.get_mut(&pos).unwrap().district = Some(Name::new(kind));
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(Name::new(kind), pos);
}

#[test]
fn registered_independent_opt_ins() {
    use super::super::test_support::opt_in_off_in_both_controllers as check;
    check("builder-workforce-recovery", |ai| {
        ai.builder_workforce_recovery
    });
    check("culture-building-catchup", |ai| ai.culture_building_catchup);
    check("expansion-best-idle-city", |ai| ai.expansion_best_idle_city);
    check("research-building-catchup", |ai| {
        ai.research_building_catchup
    });
    check("trade-building-before-bankruptcy", |ai| {
        ai.trade_building_before_bankruptcy
    });
}

#[test]
fn culture_debt_issues_one_legal_order_and_stops_at_parity() {
    let (mut g, mut ai, plan, cid) = board();
    rival_yield(&mut g, 100.0, 0.0);
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    ai.enable_culture_building_catchup();
    let (_, item, debt) = ai
        .higher_level_investment_target(&g, 0, &plan)
        .expect("culture response");
    assert_eq!(debt, Debt::Culture);
    assert_eq!(
        item,
        Item::Building {
            building: crate::name!("monument")
        }
    );
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    g.cities.get_mut(&cid).unwrap().queue.clear();
    rival_yield(&mut g, 0.0, 0.0);
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
}

#[test]
fn reservations_respect_threat_commitment_recovery_and_clock() {
    let (mut g, mut ai, mut plan, cid) = board();
    ai.enable_culture_building_catchup();
    rival_yield(&mut g, 100.0, 0.0);
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_some());
    plan.threatened_city = Some(cid);
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    plan.threatened_city = None;
    plan.strategy = GrandStrategy::Recovery;
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    plan.strategy = GrandStrategy::Expansion;
    g.cities.get_mut(&cid).unwrap().last_attacked = g.turn;
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    g.cities.get_mut(&cid).unwrap().last_attacked = 0;
    g.cities.get_mut(&cid).unwrap().queue.push(Item::Unit {
        unit: crate::name!("warrior"),
    });
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert!(matches!(g.cities[&cid].queue.first(), Some(Item::Unit { unit }) if unit == "warrior"));
    g.cities.get_mut(&cid).unwrap().queue.clear();
    g.turn = 249;
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
}

#[test]
fn research_catchup_completes_an_existing_campus() {
    let (mut g, mut ai, plan, cid) = board();
    ai.enable_research_building_catchup();
    rival_yield(&mut g, 0.0, 100.0);
    g.players[0].techs.insert(crate::name!("writing"));
    district(&mut g, cid, "campus");
    let (_, item, debt) = ai
        .higher_level_investment_target(&g, 0, &plan)
        .expect("library response");
    assert_eq!(debt, Debt::Research);
    assert_eq!(
        item,
        Item::Building {
            building: crate::name!("library")
        }
    );
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn trade_catchup_needs_filled_capacity_and_low_income() {
    let (mut g, mut ai, plan, cid) = board();
    ai.enable_trade_building_before_bankruptcy();
    g.players[0].techs.insert(crate::name!("currency"));
    g.players[0].gold_per_turn = 1.0;
    district(&mut g, cid, "commercial_hub");
    std::sync::Arc::make_mut(&mut g.observed_trade_capacity).insert(0, 1);
    assert!(
        ai.higher_level_investment_target(&g, 0, &plan).is_none(),
        "fill the existing slot first"
    );
    // A route unit in another city's queue counts as committed capacity.
    let other = g.found_city_for(0, (10, 10), None);
    g.cities.get_mut(&other).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("trader"),
    }];
    let (_, item, debt) = ai
        .higher_level_investment_target(&g, 0, &plan)
        .expect("market response");
    assert_eq!(debt, Debt::Trade);
    assert!(matches!(&item, Item::Building { building } if building == "market"));
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
    g.cities.get_mut(&cid).unwrap().queue.clear();
    g.players[0].gold_per_turn = 20.0;
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
}

#[test]
fn builder_recovery_needs_local_work_and_no_existing_builder() {
    let (mut g, mut ai, plan, cid) = board();
    ai.enable_builder_workforce_recovery();
    g.turn = 1;
    let other = g.found_city_for(0, (10, 10), None);
    g.cities.get_mut(&other).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("warrior"),
    }];
    // Put a legal farm job adjacent to the launch city.
    let pos = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|p| *p != g.cities[&cid].pos)
        .unwrap();
    let tile = g.map.tiles.get_mut(&pos).unwrap();
    tile.terrain = crate::name!("grassland");
    tile.feature = None;
    tile.improvement = None;
    tile.resource = None;
    tile.district = None;
    let (_, item, debt) = ai
        .higher_level_investment_target(&g, 0, &plan)
        .expect("replacement builder");
    assert_eq!(debt, Debt::Builder);
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
}

#[test]
fn expansion_requires_pace_and_a_real_site_and_stops_with_a_walker() {
    let (mut g, mut ai, plan, cid) = board();
    ai.enable_expansion_best_idle_city();
    g.cities.get_mut(&cid).unwrap().pop = 4;
    // The settlement atlas reads explored tiles.
    let positions: Vec<_> = g.map.tiles.keys().copied().collect();
    for pos in positions {
        g.players[0].explored.insert(pos);
    }
    let (_, item, debt) = ai
        .higher_level_investment_target(&g, 0, &plan)
        .expect("settler response");
    assert_eq!(debt, Debt::Expansion);
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    g.cities.get_mut(&cid).unwrap().queue.clear();
    g.turn = 61;
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
}
