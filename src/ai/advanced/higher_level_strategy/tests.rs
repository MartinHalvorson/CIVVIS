use super::*;
use crate::name::Name;
use crate::setup::GameSpeed;

fn board() -> (Game, AdvancedAi, StrategicPlan, u32) {
    board_with_players(2)
}

fn board_with_players(players: usize) -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new(players, 32, 22, 71, 250, 0);
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
    for v2 in [false, true] {
        let (mut g, mut ai, plan, cid) = board();
        rival_yield(&mut g, 100.0, 0.0);
        assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
        if v2 {
            ai.enable_culture_building_catchup_2();
        } else {
            ai.enable_culture_building_catchup();
        }
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
}

#[test]
fn reservations_respect_threat_commitment_recovery_and_clock() {
    for v2 in [false, true] {
        let (mut g, mut ai, mut plan, cid) = board();
        if v2 {
            ai.enable_culture_building_catchup_2();
        } else {
            ai.enable_culture_building_catchup();
        }
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
        assert!(
            matches!(g.cities[&cid].queue.first(), Some(Item::Unit { unit }) if unit == "warrior")
        );
        g.cities.get_mut(&cid).unwrap().queue.clear();
        g.turn = 249;
        assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    }
}

#[test]
fn research_catchup_completes_an_existing_campus() {
    for v2 in [false, true] {
        let (mut g, mut ai, plan, cid) = board();
        if v2 {
            ai.enable_research_building_catchup_2();
        } else {
            ai.enable_research_building_catchup();
        }
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
}

#[test]
fn trade_catchup_needs_filled_capacity_and_low_income() {
    for v2 in [false, true] {
        let (mut g, mut ai, plan, cid) = board();
        if v2 {
            ai.enable_trade_building_before_bankruptcy_2();
        } else {
            ai.enable_trade_building_before_bankruptcy();
        }
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
}

#[test]
fn builder_recovery_needs_local_work_and_no_existing_builder() {
    for v2 in [false, true] {
        let (mut g, mut ai, plan, cid) = board();
        if v2 {
            ai.enable_builder_workforce_recovery_2();
        } else {
            ai.enable_builder_workforce_recovery();
        }
        g.turn = 1;
        let other = g.found_city_for(0, (10, 10), None);
        g.cities.get_mut(&other).unwrap().queue = vec![Item::Unit {
            unit: crate::name!("warrior"),
        }];
        // Put a legal farm job adjacent to the launch city.
        let positions: Vec<_> = g.cities[&cid]
            .owned_tiles
            .iter()
            .copied()
            .filter(|p| *p != g.cities[&cid].pos)
            .take(3)
            .collect();
        for pos in positions {
            let tile = g.map.tiles.get_mut(&pos).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.hills = false;
            tile.feature = None;
            tile.improvement = None;
            tile.resource = None;
            tile.district = None;
        }
        let (_, item, debt) = ai
            .higher_level_investment_target(&g, 0, &plan)
            .expect("replacement builder");
        assert_eq!(debt, Debt::Builder);
        ai.reserve_higher_level_investment(&mut g, 0, &plan);
        assert_eq!(g.cities[&cid].queue.first(), Some(&item));
        assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    }
}

#[test]
fn expansion_requires_pace_and_a_real_site_and_stops_with_a_walker() {
    for v2 in [false, true] {
        let (mut g, mut ai, plan, cid) = board();
        if v2 {
            ai.enable_expansion_best_idle_city_2();
        } else {
            ai.enable_expansion_best_idle_city();
        }
        g.cities.get_mut(&cid).unwrap().pop = 4;
        g.turn = 25;
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
}

#[test]
fn disciplined_variants_are_registered_and_replace_their_family() {
    use super::super::test_support::opt_in_off_in_both_controllers as check;
    check("builder-workforce-recovery-2", |a| {
        a.builder_workforce_recovery_2
    });
    check("culture-building-catchup-2", |a| {
        a.culture_building_catchup_2
    });
    check("expansion-best-idle-city-2", |a| {
        a.expansion_best_idle_city_2
    });
    check("research-building-catchup-2", |a| {
        a.research_building_catchup_2
    });
    check("trade-building-before-bankruptcy-2", |a| {
        a.trade_building_before_bankruptcy_2
    });
    let mut ai = AdvancedAi::new();
    for debt in [
        Debt::Builder,
        Debt::Culture,
        Debt::Expansion,
        Debt::Research,
        Debt::Trade,
    ] {
        let old = debt.tag(&ai);
        let new = format!("{old}-2");
        let original = super::super::genes::GENES
            .iter()
            .find(|g| g.tag == old)
            .unwrap();
        let variant = super::super::genes::GENES
            .iter()
            .find(|g| g.tag == new)
            .unwrap();
        (original.enable)(&mut ai);
        (variant.enable)(&mut ai);
        assert_eq!(debt.tag(&ai), new);
        (original.enable)(&mut ai);
        assert_eq!(debt.tag(&ai), old);
    }
}

#[test]
fn expansion_v2_rejects_late_or_population_poor_launches() {
    let (mut g, mut ai, plan, cid) = board();
    g.cities.get_mut(&cid).unwrap().pop = 4;
    let positions: Vec<_> = g.map.tiles.keys().copied().collect();
    for pos in positions {
        g.players[0].explored.insert(pos);
    }
    g.turn = 59;
    ai.enable_expansion_best_idle_city();
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_some());
    ai.enable_expansion_best_idle_city_2();
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    g.turn = 25;
    g.cities.get_mut(&cid).unwrap().pop = 3;
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    g.cities.get_mut(&cid).unwrap().pop = 4;
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_some());
}

#[test]
fn culture_v2_does_not_chase_a_single_specialist() {
    let (mut g, mut ai, plan, _) = board_with_players(4);
    for pid in 1..4 {
        g.record_contact(0, pid);
        std::sync::Arc::make_mut(&mut g.observed_yield_adjustments).insert(
            pid,
            crate::rules::Yields {
                culture: if pid == 3 { 500.0 } else { 20.0 },
                ..Default::default()
            },
        );
    }
    std::sync::Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        0,
        crate::rules::Yields {
            culture: 20.0,
            ..Default::default()
        },
    );
    ai.enable_culture_building_catchup();
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_some());
    ai.enable_culture_building_catchup_2();
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    std::sync::Arc::make_mut(&mut g.observed_yield_adjustments)
        .get_mut(&2)
        .unwrap()
        .culture = 100.0;
    assert!(
        ai.higher_level_investment_target(&g, 0, &plan).is_some(),
        "a broad deficit still earns investment"
    );
}

#[test]
fn research_v2_preserves_the_spaceport_city_between_launches() {
    let (mut g, mut ai, plan, cid) = board();
    rival_yield(&mut g, 0.0, 100.0);
    g.players[0].techs.insert(crate::name!("writing"));
    district(&mut g, cid, "campus");
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), (0, 0));
    ai.enable_research_building_catchup();
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_some());
    ai.enable_research_building_catchup_2();
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert!(g.cities[&cid].queue.is_empty());
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .remove(crate::name!("spaceport"));
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert!(
        matches!(g.cities[&cid].queue.first(), Some(Item::Building { building }) if building == "library")
    );
}

#[test]
fn trade_v2_budgets_the_building_and_trader_before_route_income() {
    let (mut g, mut ai, _, cid) = board();
    ai.enable_trade_building_before_bankruptcy_2();
    let market = Item::Building {
        building: crate::name!("market"),
    };
    g.players[0].gold = 1.0;
    g.players[0].gold_per_turn = -1.0;
    assert!(!ai.disciplined_investment_admitted(&g, 0, cid, &market, Debt::Trade, 5.0));
    g.players[0].gold = 500.0;
    assert!(ai.disciplined_investment_admitted(&g, 0, cid, &market, Debt::Trade, 5.0));
    g.turn = 249;
    assert!(!ai.disciplined_investment_admitted(&g, 0, cid, &market, Debt::Trade, 5.0));
    ai.enable_trade_building_before_bankruptcy();
    assert!(
        ai.disciplined_investment_admitted(&g, 0, cid, &market, Debt::Trade, 5.0),
        "v1 admission is unchanged"
    );
}

#[test]
fn builder_v2_needs_three_distinct_jobs_not_three_improvement_choices() {
    let (mut g, mut ai, _, cid) = board();
    ai.enable_builder_workforce_recovery_2();
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let positions: Vec<_> = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .filter(|p| *p != g.cities[&cid].pos)
        .collect();
    for pos in &positions {
        g.map.tiles.get_mut(pos).unwrap().improvement = Some(crate::name!("farm"));
    }
    for (index, pos) in positions.iter().take(3).enumerate() {
        let tile = g.map.tiles.get_mut(pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        assert_eq!(
            ai.disciplined_investment_admitted(&g, 0, cid, &builder, Debt::Builder, 5.0),
            index == 2
        );
    }
}

// ---------------------- `expansion-scales-with-difficulty`: the wide cadence
//
// Filed here rather than beside the gene's own module because this is the code
// path the gene widens: same debt, same idle-queue rule, same ranking, same
// admission tests. See `advanced/expansion_scales_with_difficulty.rs`.

use super::super::expansion_scales_with_difficulty::WIDE_PARALLEL_SETTLERS;

fn wide() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_expansion_scales_with_difficulty();
    ai
}

/// A capital plus one other city, everything explored, both queues empty.
fn two_cities() -> (Game, u32, u32) {
    let mut g = Game::new(2, 32, 22, 71, 250, 0);
    g.game_speed = GameSpeed::Online;
    g.difficulty = "emperor".to_string();
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let start = g.units[&settler].pos;
    let capital = g.found_city_for(0, start, None);
    g.remove_unit(settler);
    let second = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.wdist(*pos, start) == 5)
        .find(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile)))
        .expect("a land tile five steps out");
    let second = g.found_city_for(0, second, None);
    for cid in [capital, second] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.queue.clear();
        city.buildings.clear();
        city.pop = 5;
    }
    let positions: Vec<_> = g.map.tiles.keys().copied().collect();
    for pos in positions {
        g.players[0].explored.insert(pos);
    }
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 10.0;
    g.turn = 25;
    g.record_contact(0, 1);
    (g, capital, second)
}

fn wide_plan(desired: usize) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: desired,
        assessed_turn: 25,
        rush: false,
    }
}

#[test]
fn a_busy_capital_hands_the_settler_to_the_best_other_city() {
    let (mut g, capital, second) = two_cities();
    let plan = wide_plan(9);
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    g.cities.get_mut(&capital).unwrap().queue.push(settler);

    // The shipped reservation reads a Settler queued anywhere as the debt
    // already serviced, so version two asks for nothing.
    let mut v2 = AdvancedAi::new();
    v2.enable_expansion_best_idle_city_2();
    assert!(v2.higher_level_investment_target(&g, 0, &plan).is_none());

    // The rung-scaled cadence takes the best OTHER idle city instead.
    let (cid, item, debt) = wide()
        .higher_level_investment_target(&g, 0, &plan)
        .expect("a second settler factory");
    assert_eq!(debt, Debt::Expansion);
    assert_eq!(cid, second, "never the busy capital, which is not idle");
    assert!(matches!(&item, Item::Unit { unit } if unit == "settler"));
}

#[test]
fn a_capital_busy_with_a_district_is_busy_too_and_an_idle_one_is_not() {
    let (mut g, capital, _) = two_cities();
    let plan = wide_plan(9);
    assert!(
        !AdvancedAi::expansion_wide_capital_is_busy(&g, 0),
        "an empty capital queue is not busy"
    );
    let pos = g.cities[&capital]
        .owned_tiles
        .iter()
        .copied()
        .find(|p| *p != g.cities[&capital].pos)
        .unwrap();
    g.cities.get_mut(&capital).unwrap().queue.push(Item::District {
        district: crate::name!("campus"),
        pos,
    });
    assert!(AdvancedAi::expansion_wide_capital_is_busy(&g, 0));
    // A cheap building in the capital is not the stall this gene clears.
    g.cities.get_mut(&capital).unwrap().queue.clear();
    g.cities.get_mut(&capital).unwrap().queue.push(Item::Building {
        building: crate::name!("monument"),
    });
    assert!(!AdvancedAi::expansion_wide_capital_is_busy(&g, 0));
    // And with the capital idle, both genes agree on the capital itself.
    g.cities.get_mut(&capital).unwrap().queue.clear();
    let (cid, _, debt) = wide()
        .higher_level_investment_target(&g, 0, &plan)
        .expect("the ordinary reservation");
    assert_eq!(debt, Debt::Expansion);
    assert_eq!(cid, capital, "the capital is still the best factory");
}

#[test]
fn the_cadence_admits_at_most_two_walkers_and_only_behind_a_busy_capital() {
    let (mut g, capital, _) = two_cities();
    let ai = wide();
    let off = AdvancedAi::new();
    // No walker: every arm admits, exactly as the shipped rule does.
    assert!(ai.expansion_wide_cadence_admits(&g, 0, 0));
    assert!(off.expansion_wide_cadence_admits(&g, 0, 0));
    // One walker and an idle capital: the shipped rule stands.
    assert!(!ai.expansion_wide_cadence_admits(&g, 0, 1));
    g.cities
        .get_mut(&capital)
        .unwrap()
        .queue
        .push(Item::Unit {
            unit: crate::name!("settler"),
        });
    assert!(ai.expansion_wide_cadence_admits(&g, 0, 1), "the stall case");
    assert!(
        !ai.expansion_wide_cadence_admits(&g, 0, WIDE_PARALLEL_SETTLERS),
        "two walkers is the ceiling"
    );
    assert!(
        !off.expansion_wide_cadence_admits(&g, 0, 1),
        "off, a walker always closes the reservation"
    );
}

#[test]
fn the_cadence_pauses_when_no_safe_site_is_left() {
    let (mut g, capital, _) = two_cities();
    let plan = wide_plan(9);
    g.cities.get_mut(&capital).unwrap().queue.push(Item::Unit {
        unit: crate::name!("settler"),
    });
    assert!(
        wide().higher_level_investment_target(&g, 0, &plan).is_some(),
        "the site gate passes on an open board"
    );
    // Drown everything outside the two cities' own rings. There is now no
    // legal seat inside the safe radius at all, and no Shipbuilding to widen
    // the search, so the walker-aware site gate holds: the cadence asks for
    // nothing rather than sending a Settler into the fog.
    let home = g.cities[&capital].pos;
    let positions: Vec<_> = g.map.tiles.keys().copied().collect();
    for pos in positions {
        if g.wdist(pos, home) <= 2 {
            continue;
        }
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("ocean");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
    }
    assert!(
        g.player_city_ids(0)
            .into_iter()
            .all(|cid| AdvancedAi::new()
                .settler_site_gate(&g, 0, g.cities[&cid].pos, 0)
                .is_err()),
        "no seat is left to walk to"
    );
    assert!(wide().higher_level_investment_target(&g, 0, &plan).is_none());
}
