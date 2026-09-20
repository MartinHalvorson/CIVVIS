use super::super::*;

fn siege_gap_case() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 40, 26, 79_301, 500, 0, false);
    g.current = 0;
    let settlers: Vec<_> = g
        .units
        .values()
        .filter(|u| u.kind == "settler")
        .map(|u| (u.owner, u.pos))
        .collect();
    for (owner, pos) in settlers {
        g.found_city_for(owner, pos, None);
    }
    let home = g.player_city_ids(0)[0];
    let target = g.player_city_ids(1)[0];
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    let pos = g.cities[&home].pos;
    for kind in ["warrior", "warrior", "archer", "archer"] {
        g.spawn_unit(kind, 0, pos);
    }
    g.spawn_unit("builder", 0, pos);
    g.players[0]
        .techs
        .extend(["archery", "masonry", "engineering"].map(crate::name::Name::new));
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 30.0;
    g.cities.get_mut(&target).unwrap().wall_hp = 200;
    g.cities
        .get_mut(&target)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    g.cities
        .get_mut(&target)
        .unwrap()
        .buildings
        .push(crate::name!("medieval_walls"));
    g.turn = 125;
    g.at_war.insert((0, 1));
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, home, target)
}

#[test]
fn an_expanding_empire_equips_its_active_siege_before_the_army_ceiling() {
    assert_war_strategy_equips_siege(GrandStrategy::Expansion);
}

#[test]
fn a_recovering_empire_keeps_the_first_siege_weapon_requirement() {
    assert_war_strategy_equips_siege(GrandStrategy::Recovery);
}

fn assert_war_strategy_equips_siege(strategy: GrandStrategy) {
    let (mut g, mut ai, mut plan, home, _) = siege_gap_case();
    plan.strategy = strategy;
    let catapult = Item::Unit {
        unit: crate::name!("catapult"),
    };
    assert!(g.can_produce(0, home, &catapult));
    let counts = ai.counts(&g, 0);
    assert!(
        ai.production_value(&g, 0, home, &catapult, &plan, &counts) > 0.0,
        "an active walled siege still needs its first weapon after a strategy change"
    );
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&home].queue.first(), Some(&catapult));
}

#[test]
fn an_expansion_plan_does_not_duplicate_an_existing_siege_order() {
    let (mut g, ai, mut plan, home, _) = siege_gap_case();
    plan.strategy = GrandStrategy::Expansion;
    g.cities.get_mut(&home).unwrap().queue.push(Item::Unit {
        unit: crate::name!("catapult"),
    });
    assert!(!ai.missing_domination_siege(
        &g,
        0,
        home,
        &plan,
        &ai.counts(&g, 0),
        &g.rules.units["catapult"]
    ));
}

#[test]
fn the_first_wall_breaker_outweighs_routine_growth_infrastructure() {
    let (mut g, mut ai, mut plan, home, _) = siege_gap_case();
    plan.strategy = GrandStrategy::Expansion;
    g.cities.get_mut(&home).unwrap().pop = 6;
    g.players[0].techs.insert(crate::name!("pottery"));
    let catapult = Item::Unit {
        unit: crate::name!("catapult"),
    };
    let granary = Item::Building {
        building: crate::name!("granary"),
    };
    assert!(g.can_produce(0, home, &catapult));
    assert!(g.can_produce(0, home, &granary));
    let counts = ai.counts(&g, 0);
    let weapon_value = ai.production_value(&g, 0, home, &catapult, &plan, &counts);
    let growth_value = ai.production_value(&g, 0, home, &granary, &plan, &counts);
    assert!(
        weapon_value > growth_value,
        "first weapon {weapon_value} must outrank routine growth {growth_value}"
    );
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&home].queue.first(), Some(&catapult));
    for strategy in [GrandStrategy::Recovery, GrandStrategy::Expansion] {
        g.turn += 1;
        plan.strategy = strategy;
        plan.assessed_turn = g.turn;
        ai.advanced_production(&mut g, 0, &plan, false);
        assert_eq!(
            g.cities[&home].queue.first(),
            Some(&catapult),
            "the retained weapon must not close its own demand on a replan"
        );
    }
}

fn assert_queued_siege_survives_a_targetless_replan(strategy: GrandStrategy) {
    let (mut g, mut ai, mut plan, home, _) = siege_gap_case();
    let catapult = Item::Unit {
        unit: crate::name!("catapult"),
    };
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&home].queue.first(), Some(&catapult));
    assert_eq!(g.item_invested_production(home, &catapult), 0.0);
    plan.strategy = strategy;
    plan.target_city = None;
    g.turn += 1;
    plan.assessed_turn = g.turn;
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(
        g.cities[&home].queue.first(),
        Some(&catapult),
        "losing the current city target does not remove the hostile walls or the first weapon requirement"
    );
}

#[test]
fn recovery_keeps_the_queued_first_weapon_when_its_city_target_disappears() {
    assert_queued_siege_survives_a_targetless_replan(GrandStrategy::Recovery);
}

#[test]
fn expansion_keeps_the_queued_first_weapon_when_its_city_target_disappears() {
    assert_queued_siege_survives_a_targetless_replan(GrandStrategy::Expansion);
}

#[test]
fn a_targetless_plan_does_not_start_a_new_siege_reservation() {
    let (g, ai, mut plan, home, _) = siege_gap_case();
    plan.target_city = None;
    plan.strategy = GrandStrategy::Recovery;
    assert_eq!(
        ai.production_value(
            &g,
            0,
            home,
            &Item::Unit {
                unit: crate::name!("catapult")
            },
            &plan,
            &ai.counts(&g, 0),
        ),
        -2_000.0,
    );
}

#[test]
fn a_targetless_queued_weapon_loses_priority_when_the_requirement_ends() {
    for case in 0..4 {
        let (mut g, ai, mut plan, home, target) = siege_gap_case();
        let catapult = Item::Unit {
            unit: crate::name!("catapult"),
        };
        g.cities
            .get_mut(&home)
            .unwrap()
            .queue
            .push(catapult.clone());
        plan.target_city = None;
        plan.strategy = GrandStrategy::Recovery;
        match case {
            0 => g.at_war.clear(),
            1 => g.cities.get_mut(&target).unwrap().wall_hp = 0,
            2 => {
                g.spawn_unit("catapult", 0, g.cities[&home].pos);
            }
            _ => g.victory_conditions.domination = false,
        }
        let counts = ai.counts_without_city_queue(&g, 0, home);
        assert_eq!(
            ai.production_value(&g, 0, home, &catapult, &plan, &counts),
            -2_000.0,
            "ended reservation case {case}",
        );
    }
}
